mod apify;
mod resend;

use async_trait::async_trait;

use crate::models::{ProviderConfig, UsageSnapshot};

pub use apify::ApifyProvider;
pub use resend::ResendProvider;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("this provider is not implemented yet")]
    Unsupported,
    #[error("provider request failed")]
    Request,
    #[error(
        "Your Resend API key has Sending access only. Please create a Full access API key to retrieve usage statistics."
    )]
    ResendSendingAccessOnly,
    #[error("provider returned an invalid usage response")]
    InvalidResponse,
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &str,
    ) -> Result<UsageSnapshot, ProviderError>;
}

#[derive(Default)]
pub struct ProviderRegistry;

impl ProviderRegistry {
    pub async fn collect(
        &self,
        config: &ProviderConfig,
        secret: &str,
    ) -> Result<UsageSnapshot, ProviderError> {
        match config.provider_type.as_str() {
            "apify" => ApifyProvider.collect(config, secret).await,
            "resend" => ResendProvider.collect(config, secret).await,
            _ => Err(ProviderError::Unsupported),
        }
    }
}
