use async_trait::async_trait;

use crate::models::{Metric, ProviderConfig, UsageSnapshot};

use super::{Provider, ProviderError};

pub struct ResendProvider;

#[async_trait]
impl Provider for ResendProvider {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &str,
    ) -> Result<UsageSnapshot, ProviderError> {
        // Resend returns quota usage on ordinary authenticated API responses.
        // The response body is intentionally not read or persisted.
        let response = reqwest::Client::new()
            .get("https://api.resend.com/logs?limit=1")
            .bearer_auth(secret)
            .header("User-Agent", "rust-usage-dash/0.1")
            .send()
            .await
            .map_err(|_| ProviderError::Request)?;
        if !response.status().is_success() {
            return Err(ProviderError::Request);
        }

        let monthly = quota_header(&response, "x-resend-monthly-quota")
            .ok_or(ProviderError::InvalidResponse)?;
        let mut metrics = vec![Metric {
            id: "monthly_email_quota".to_owned(),
            label: "Monthly emails sent".to_owned(),
            used: monthly,
            limit: None,
            unit: "emails".to_owned(),
        }];
        if let Some(daily) = quota_header(&response, "x-resend-daily-quota") {
            metrics.push(Metric {
                id: "daily_email_quota".to_owned(),
                label: "Daily emails sent".to_owned(),
                used: daily,
                limit: None,
                unit: "emails".to_owned(),
            });
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

fn quota_header(response: &reqwest::Response, name: &str) -> Option<f64> {
    response.headers().get(name)?.to_str().ok()?.parse().ok()
}

fn unix_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
