mod gcp;

use async_trait::async_trait;

pub use gcp::GcpSecretManagerResolver;

#[async_trait]
pub trait SecretResolver: Send + Sync {
    /// Resolves a credential only for the duration of a collection request.
    async fn resolve(&self, reference: &str) -> Result<String, SecretError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret reference must use projects/<project>/secrets/<secret>/versions/<version>")]
    InvalidReference,
    #[error("Google Secret Manager could not resolve this secret reference")]
    ResolutionFailed,
}
