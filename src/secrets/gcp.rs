use async_trait::async_trait;
use tokio::process::Command;

use super::{SecretError, SecretResolver};

pub struct GcpSecretManagerResolver;

#[async_trait]
impl SecretResolver for GcpSecretManagerResolver {
    async fn resolve(&self, reference: &str) -> Result<String, SecretError> {
        let parts: Vec<_> = reference.split('/').collect();
        if parts.len() != 6
            || parts[0] != "projects"
            || parts[2] != "secrets"
            || parts[4] != "versions"
            || parts.iter().any(|part| part.is_empty())
        {
            return Err(SecretError::InvalidReference);
        }
        let output = Command::new("gcloud")
            .args([
                "secrets",
                "versions",
                "access",
                parts[5],
                "--secret",
                parts[3],
                "--project",
                parts[1],
                "--quiet",
            ])
            .output()
            .await
            .map_err(|_| SecretError::ResolutionFailed)?;
        if !output.status.success() {
            return Err(SecretError::ResolutionFailed);
        }
        // Do not log this value. It is dropped once the collector returns.
        String::from_utf8(output.stdout)
            .map(|value| value.trim_end_matches(['\r', '\n']).to_owned())
            .map_err(|_| SecretError::ResolutionFailed)
    }
}
