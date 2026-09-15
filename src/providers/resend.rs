use async_trait::async_trait;
use chrono::{Datelike, Utc};
use serde::Deserialize;

use crate::models::{Metric, ProviderConfig, UsageSnapshot};

use super::{Provider, ProviderError};

pub struct ResendProvider;

#[derive(Deserialize)]
struct EmailMetricsResponse {
    totals: EmailMetricsTotals,
}

#[derive(Deserialize)]
struct EmailMetricsTotals {
    sent: Option<f64>,
    received: Option<f64>,
}

#[derive(Deserialize)]
struct ResendErrorResponse {
    name: Option<String>,
}

#[async_trait]
impl Provider for ResendProvider {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &str,
    ) -> Result<UsageSnapshot, ProviderError> {
        let end_date = Utc::now().date_naive();
        // The API does not expose an account billing-cycle start. Use the
        // current calendar month: its first day through today.
        let start_date = end_date.with_day(1).ok_or(ProviderError::InvalidResponse)?;
        let response = request(
            reqwest::Client::new()
                .get("https://api.resend.com/emails/metrics")
                .query(&[
                    ("start_date", start_date.to_string()),
                    ("end_date", end_date.to_string()),
                    ("metrics", "sent,received".to_owned()),
                ])
                .bearer_auth(secret)
                .header("User-Agent", "rust-usage-dash/0.1")
                .send(),
            "/emails/metrics",
        )
        .await?;
        let response = successful_response(response, "/emails/metrics").await?;
        eprintln!(
            "event=resend_usage_collected endpoint=/emails/metrics status={} metrics=sent,received start_date={} end_date={}",
            response.status().as_u16(),
            start_date,
            end_date,
        );
        let response = response.json::<EmailMetricsResponse>().await.map_err(|_| {
            eprintln!("event=resend_usage_metrics_parse_failed endpoint=/emails/metrics");
            ProviderError::InvalidResponse
        })?;

        let sent = response.totals.sent.unwrap_or(0.0);
        let received = response.totals.received.unwrap_or(0.0);
        let used = sent + received;
        let limit = config.monthly_quota as f64;
        let metrics = vec![
            metric(
                "emails_sent_current_month",
                "Emails sent (current month)",
                sent,
            ),
            metric(
                "emails_received_current_month",
                "Emails received (current month)",
                received,
            ),
            Metric {
                id: "monthly_email_quota".to_owned(),
                label: format!("Current month usage ({})", config.plan),
                used,
                limit: Some(limit),
                unit: "emails".to_owned(),
            },
            metric(
                "monthly_email_quota_remaining",
                "Monthly quota remaining",
                config.monthly_quota.saturating_sub(used as i64) as f64,
            ),
            metric(
                "monthly_email_quota_used_percent",
                "Monthly quota used",
                used / limit * 100.0,
            ),
        ];

        Ok(UsageSnapshot {
            provider_id: config.id.clone(),
            timestamp: unix_timestamp(),
            status: "ok".to_owned(),
            cost: None,
            currency: None,
            metrics,
        })
    }
}

fn metric(id: &str, label: &str, used: f64) -> Metric {
    Metric {
        id: id.to_owned(),
        label: label.to_owned(),
        used,
        limit: None,
        unit: "emails".to_owned(),
    }
}

async fn request(
    request: impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>>,
    endpoint: &str,
) -> Result<reqwest::Response, ProviderError> {
    request.await.map_err(|error| {
        eprintln!(
            "event=resend_usage_request_failed endpoint={} transport={} timeout={} connect={} request={} status={}",
            endpoint,
            transport_category(&error),
            error.is_timeout(),
            error.is_connect(),
            error.is_request(),
            error.status().map(|status| status.as_u16()).unwrap_or(0),
        );
        ProviderError::Request
    })
}

async fn successful_response(
    response: reqwest::Response,
    endpoint: &str,
) -> Result<reqwest::Response, ProviderError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let error_name = response
        .json::<ResendErrorResponse>()
        .await
        .ok()
        .and_then(|error| error.name);
    eprintln!(
        "event=resend_usage_response_failed endpoint={} status={} error_kind={}",
        endpoint,
        status.as_u16(),
        error_name.as_deref().unwrap_or("unavailable"),
    );
    if status == reqwest::StatusCode::UNAUTHORIZED
        && error_name.as_deref() == Some("restricted_api_key")
    {
        return Err(ProviderError::ResendSendingAccessOnly);
    }
    Err(ProviderError::Request)
}

fn transport_category(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "timeout"
    } else if error.is_connect() {
        "connect"
    } else if error.is_request() {
        "request"
    } else {
        "other"
    }
}

fn unix_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
