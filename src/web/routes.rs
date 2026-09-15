use std::sync::Arc;

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};

use crate::{
    models::{NewProvider, ProviderConfig, UsageSnapshot},
    web::AppState,
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(index))
        .route("/health", get(|| async { "ok" }))
        .route("/providers", post(create_provider))
        .route("/providers/new", get(new_provider_form))
        .route("/providers/{id}/refresh", post(refresh_provider))
}

async fn index(State(state): State<Arc<AppState>>) -> Result<Html<String>, AppError> {
    let provider_list_html = render_cards(&state)?;
    Ok(Html(
        IndexTemplate {
            provider_list_html: &provider_list_html,
        }
        .render()?,
    ))
}

async fn new_provider_form() -> Result<Html<String>, AppError> {
    Ok(Html(NewProviderTemplate.render()?))
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    Form(input): Form<NewProvider>,
) -> Result<Html<String>, AppError> {
    if !matches!(
        input.provider_type.as_str(),
        "apify" | "openai" | "neon" | "upstash" | "resend"
    ) || input.display_name.trim().is_empty()
        || input.secret_ref.trim().is_empty()
    {
        return Err(AppError::BadRequest);
    }
    state.database.add_provider(input)?;
    let cards_html = render_cards(&state)?;
    Ok(Html(ProviderListTemplate { cards_html }.render()?))
}

async fn refresh_provider(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, AppError> {
    let provider = state
        .database
        .find_provider(&id)?
        .ok_or(AppError::NotFound)?;
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

#[derive(Template)]
#[template(path = "pages/index.html")]
struct IndexTemplate<'a> {
    provider_list_html: &'a str,
}

#[derive(Template)]
#[template(path = "partials/provider_list.html")]
struct ProviderListTemplate {
    cards_html: String,
}

#[derive(Template)]
#[template(path = "partials/provider_card.html")]
struct ProviderCardTemplate {
    provider: ProviderConfig,
    snapshot: Option<UsageSnapshot>,
    message: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/new_provider.html")]
struct NewProviderTemplate;

fn render_cards(state: &AppState) -> Result<String, AppError> {
    let cards = state
        .database
        .list_providers()?
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
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(cards.join("\n"))
}

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error("database error")]
    Database(#[from] rusqlite::Error),
    #[error("template error")]
    Template(#[from] askama::Error),
    #[error("invalid provider configuration")]
    BadRequest,
    #[error("provider was not found")]
    NotFound,
}
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
