use async_trait::async_trait;
use serde_json::Value;

use crate::models::{Metric, ProviderConfig, UsageSnapshot};

use super::{Provider, ProviderError};

pub struct ApifyProvider;

#[async_trait]
impl Provider for ApifyProvider {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &str,
    ) -> Result<UsageSnapshot, ProviderError> {
        let response = reqwest::Client::new()
            .get("https://api.apify.com/v2/users/me/usage/monthly")
            .bearer_auth(secret)
            .send()
            .await
            .map_err(|_| ProviderError::Request)?;
        if !response.status().is_success() {
            return Err(ProviderError::Request);
        }
        let payload: Value = response
            .json()
            .await
            .map_err(|_| ProviderError::InvalidResponse)?;
        let data = payload.get("data").unwrap_or(&payload);
        let used = data
            .get("totalUsageCredits")
            .or_else(|| data.get("usageCredits"))
            .and_then(Value::as_f64)
            .ok_or(ProviderError::InvalidResponse)?;
        let limit = data
            .get("maxMonthlyUsageCredits")
            .or_else(|| data.get("limit"))
            .and_then(Value::as_f64);
        Ok(UsageSnapshot {
            provider_id: config.id.clone(),
            timestamp: unix_timestamp(),
            status: "ok".to_owned(),
            cost: None,
            currency: None,
            metrics: vec![Metric {
                id: "usage_credits".to_owned(),
                label: "Usage credits".to_owned(),
                used,
                limit,
                unit: "credits".to_owned(),
            }],
        })
    }
}

fn unix_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
