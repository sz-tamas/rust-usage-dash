use std::sync::Arc;

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post},
};

use crate::{
    models::{Account, NewAccount, NewProvider, Onboarding, ProviderConfig, UsageSnapshot},
    secrets::{begin_authentication, check_application_default_credentials},
    web::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/health", get(|| async { "ok" }))
        .route("/onboarding/account", post(create_account))
        .route("/onboarding/auth", post(start_auth))
        .route("/onboarding/auth/check", post(check_auth))
        .route("/onboarding/status", get(onboarding_status))
        .route("/onboarding/alerts/skip", post(skip_alerts))
        .route("/providers", post(create_provider))
        .route("/providers/new", get(new_provider_form))
        .route("/providers/{id}/refresh", post(refresh_provider))
}

async fn index(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    render_dashboard(&state)
}

async fn onboarding_status(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let account = state.database.active_account()?;
    let onboarding = account
        .as_ref()
        .map(|item| state.database.onboarding(&item.id))
        .transpose()?;
    Ok(Html(
        OnboardingModalTemplate {
            account,
            onboarding,
        }
        .render()?,
    ))
}

async fn create_account(
    State(state): State<Arc<AppState>>,
    Form(input): Form<NewAccount>,
) -> Result<Html<String>, AppError> {
    if !valid_project_id(&input.project_id) {
        return Err(AppError::BadRequest);
    }
    let account = state.database.create_account(input)?;
    let onboarding = state.database.onboarding(&account.id)?;
    Ok(Html(
        OnboardingModalTemplate {
            account: Some(account),
            onboarding: Some(onboarding),
        }
        .render()?,
    ))
}

async fn start_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    state
        .database
        .set_auth_status(&account.id, "authenticating", None, None)?;
    tokio::spawn(async move {
        let _ = begin_authentication(&account.project_id).await;
    });
    onboarding_status(State(state)).await
}

async fn check_auth(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    match check_application_default_credentials().await {
        Ok(()) => {
            state.database.set_auth_status(
                &account.id,
                "ready",
                Some(&account.project_id),
                None,
            )?;
            state.database.set_onboarding_step(&account.id, 2)?;
        }
        Err(_) => state.database.set_auth_status(
            &account.id,
            "authenticating",
            None,
            Some("Google credentials are not ready yet. Complete sign-in, then check again."),
        )?,
    }
    onboarding_status(State(state)).await
}

async fn new_provider_form(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    Ok(Html(NewProviderTemplate.render()?))
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    Form(input): Form<NewProvider>,
) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    if account.auth_status != "ready"
        || !matches!(
            input.provider_type.as_str(),
            "apify" | "openai" | "neon" | "upstash" | "resend"
        )
        || input.display_name.trim().is_empty()
        || input.secret_ref.trim().is_empty()
    {
        return Err(AppError::BadRequest);
    }
    let is_resend = input.provider_type == "resend";
    state.database.add_provider(&account.id, input)?;
    state
        .database
        .set_onboarding_step(&account.id, if is_resend { 4 } else { 3 })?;
    render_dashboard(&state)
}

async fn skip_alerts(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let account = state
        .database
        .active_account()?
        .ok_or(AppError::BadRequest)?;
    state.database.set_onboarding_step(&account.id, 4)?;
    render_dashboard(&state)
}

async fn refresh_provider(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .database
        .find_provider(&id)?
        .ok_or(AppError::NotFound)?;
    let account = state.database.active_account()?.ok_or(AppError::NotFound)?;
    if provider.account_id != account.id {
        return Err(AppError::NotFound);
    }
    let outcome = async {
        let secret = state
            .secret_resolver
            .resolve(&provider.secret_ref)
            .await
            .map_err(|_| ())?;
        let snapshot = state
            .providers
            .collect(&provider, &secret)
            .await
            .map_err(|_| ())?;
        state.database.save_snapshot(&snapshot).map_err(|_| ())?;
        Ok::<_, ()>(snapshot)
    }
    .await;
    let (snapshot, message) = match outcome {
        Ok(snapshot) => (Some(snapshot), Some("Usage refreshed.".to_owned())),
        Err(()) => (
            state.database.latest_snapshot(&provider.id)?,
            Some(
                "Refresh failed. Check the GCP reference, ADC login, and provider access."
                    .to_owned(),
            ),
        ),
    };
    Ok(Html(
        ProviderCardTemplate {
            provider,
            snapshot,
            message,
        }
        .render()?,
    ))
}

fn render_dashboard(state: &AppState) -> Result<Html<String>, AppError> {
    let account = state.database.active_account()?;
    // Loading the project display name here ensures the authenticated project
    // metadata remains part of the dashboard state, ready for the header UI.
    let _project_name = account
        .as_ref()
        .and_then(|item| item.project_name.as_deref());
    let cards_html = match &account {
        Some(account) => render_cards(state, &account.id)?,
        None => String::new(),
    };
    let onboarding = account
        .as_ref()
        .map(|item| state.database.onboarding(&item.id))
        .transpose()?;
    Ok(Html(
        DashboardTemplate {
            account,
            onboarding,
            cards_html,
        }
        .render()?,
    ))
}

fn render_cards(state: &AppState, account_id: &str) -> Result<String, AppError> {
    state
        .database
        .list_providers(account_id)?
        .into_iter()
        .map(|provider| {
            ProviderCardTemplate {
                snapshot: state.database.latest_snapshot(&provider.id)?,
                provider,
                message: None,
            }
            .render()
            .map_err(AppError::from)
        })
        .collect::<Result<Vec<_>, AppError>>()
        .map(|cards| cards.join("\n"))
}

fn valid_project_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 63
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

#[derive(Template)]
#[template(path = "pages/index.html")]
struct DashboardTemplate {
    account: Option<Account>,
    onboarding: Option<Onboarding>,
    cards_html: String,
}
#[derive(Template)]
#[template(path = "partials/onboarding_modal.html")]
struct OnboardingModalTemplate {
    account: Option<Account>,
    onboarding: Option<Onboarding>,
}
#[derive(Template)]
#[template(path = "partials/new_provider.html")]
struct NewProviderTemplate;
#[derive(Template)]
#[template(path = "partials/provider_card.html")]
struct ProviderCardTemplate {
    provider: ProviderConfig,
    snapshot: Option<UsageSnapshot>,
    message: Option<String>,
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("database error")]
    Database(#[from] rusqlite::Error),
    #[error("template error")]
    Template(#[from] askama::Error),
    #[error("invalid request")]
    BadRequest,
    #[error("provider was not found")]
    NotFound,
}
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            Self::BadRequest => axum::http::StatusCode::BAD_REQUEST,
            Self::NotFound => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
