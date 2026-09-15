use async_trait::async_trait;
use chrono::{Duration, Utc};
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
    delivered: Option<f64>,
    bounced: Option<f64>,
    complained: Option<f64>,
    failed: Option<f64>,
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
        // The Metrics API returns aggregate, sanitized usage totals. The full
        // response body is neither logged nor persisted.
        eprintln!(
            "event=resend_usage_credential_loaded endpoint=/emails/metrics present={} length={} has_resend_prefix={} has_bearer_prefix={} has_whitespace={}",
            !secret.is_empty(),
            secret.len(),
            secret.starts_with("re_"),
            secret.starts_with("Bearer "),
            secret.chars().any(char::is_whitespace),
        );
        let end_date = Utc::now().date_naive();
        let start_date = end_date - Duration::days(config.resend_interval_days);
        let metrics = "sent,delivered,bounced,complained,failed";
        let response = reqwest::Client::new()
            .get("https://api.resend.com/emails/metrics")
            .query(&[
                ("start_date", start_date.to_string()),
                ("end_date", end_date.to_string()),
                ("metrics", metrics.to_owned()),
            ])
            .bearer_auth(secret)
            .header("User-Agent", "rust-usage-dash/0.1")
            .send()
            .await
            .map_err(|error| {
                // Do not log the error string: it can contain request context.
                // These booleans are enough to distinguish local connectivity,
                // timeout, TLS, and protocol failures without exposing secrets.
                eprintln!(
                    "event=resend_usage_request_failed endpoint=/emails/metrics transport={} timeout={} connect={} request={} status={}",
                    transport_category(&error),
                    error.is_timeout(),
                    error.is_connect(),
                    error.is_request(),
                    error.status().map(|status| status.as_u16()).unwrap_or(0),
                );
                ProviderError::Request
            })?;
        if !response.status().is_success() {
            let status = response.status();
            // Only deserialize Resend's stable error identifier. Never log or
            // persist the response body, which may contain provider context.
            let error_name = response
                .json::<ResendErrorResponse>()
                .await
                .ok()
                .and_then(|error| error.name);
            eprintln!(
                "event=resend_usage_response_failed endpoint=/emails/metrics status={} error_kind={}",
                status.as_u16(),
                error_name.as_deref().unwrap_or("unavailable"),
            );
            if status == reqwest::StatusCode::UNAUTHORIZED
                && error_name.as_deref() == Some("restricted_api_key")
            {
                return Err(ProviderError::ResendSendingAccessOnly);
            }
            return Err(ProviderError::Request);
        }

        let quota_headers = QuotaHeaders {
            monthly: quota_header(&response, "x-resend-monthly-quota"),
            daily: quota_header(&response, "x-resend-daily-quota"),
        };
        let response = response.json::<EmailMetricsResponse>().await.map_err(|_| {
            eprintln!("event=resend_usage_metrics_parse_failed endpoint=/emails/metrics");
            ProviderError::InvalidResponse
        })?;

        let mut metrics = Vec::new();
        add_metric(
            &mut metrics,
            &format!("emails_sent_{}d", config.resend_interval_days),
            &format!("Emails sent (last {} days)", config.resend_interval_days),
            response.totals.sent,
        );
        add_metric(
            &mut metrics,
            &format!("emails_delivered_{}d", config.resend_interval_days),
            &format!(
                "Emails delivered (last {} days)",
                config.resend_interval_days
            ),
            response.totals.delivered,
        );
        add_metric(
            &mut metrics,
            &format!("emails_bounced_{}d", config.resend_interval_days),
            &format!("Emails bounced (last {} days)", config.resend_interval_days),
            response.totals.bounced,
        );
        add_metric(
            &mut metrics,
            &format!("emails_complained_{}d", config.resend_interval_days),
            &format!(
                "Spam complaints (last {} days)",
                config.resend_interval_days
            ),
            response.totals.complained,
        );
        add_metric(
            &mut metrics,
            &format!("emails_failed_{}d", config.resend_interval_days),
            &format!("Emails failed (last {} days)", config.resend_interval_days),
            response.totals.failed,
        );

        if let Some(monthly) = quota_headers.monthly {
            metrics.push(Metric {
                id: "monthly_email_quota".to_owned(),
                label: "Monthly email quota usage".to_owned(),
                used: monthly,
                limit: None,
                unit: "emails".to_owned(),
            });
        }
        if let Some(daily) = quota_headers.daily {
            metrics.push(Metric {
                id: "daily_email_quota".to_owned(),
                label: "Daily emails sent".to_owned(),
                used: daily,
                limit: None,
                unit: "emails".to_owned(),
            });
        }
        if metrics.is_empty() {
            eprintln!("event=resend_usage_metrics_empty endpoint=/emails/metrics");
            return Err(ProviderError::InvalidResponse);
        }

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

struct QuotaHeaders {
    monthly: Option<f64>,
    daily: Option<f64>,
}

fn add_metric(metrics: &mut Vec<Metric>, id: &str, label: &str, value: Option<f64>) {
    if let Some(used) = value {
        metrics.push(Metric {
            id: id.to_owned(),
            label: label.to_owned(),
            used,
            limit: None,
            unit: "emails".to_owned(),
        });
    }
}

fn quota_header(response: &reqwest::Response, name: &str) -> Option<f64> {
    response.headers().get(name)?.to_str().ok()?.parse().ok()
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
