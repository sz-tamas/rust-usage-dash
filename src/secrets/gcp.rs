use async_trait::async_trait;
use tokio::process::Command;

use super::{SecretError, SecretResolver};

pub struct GcpSecretManagerResolver;

/// Opens the Google Cloud ADC browser flow. Verification is a separate user action.
pub async fn begin_authentication(project_id: &str) -> Result<(), SecretError> {
    let login = Command::new("gcloud")
        .args([
            "auth",
            "application-default",
            "login",
            "--project",
            project_id,
            "--quiet",
        ])
        .output()
        .await
        .map_err(|_| SecretError::ResolutionFailed)?;
    if login.status.success() {
        Ok(())
    } else {
        Err(SecretError::ResolutionFailed)
    }
}

/// Checks the exact ADC credential used by the collector without exposing its token.
pub async fn check_application_default_credentials() -> Result<(), SecretError> {
    let credentials = Command::new("gcloud")
        .args([
            "auth",
            "application-default",
            "print-access-token",
            "--quiet",
        ])
        .output()
        .await
        .map_err(|_| SecretError::ResolutionFailed)?;
    if !credentials.status.success() || credentials.stdout.is_empty() {
        return Err(SecretError::ResolutionFailed);
    }
    Ok(())
}

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
