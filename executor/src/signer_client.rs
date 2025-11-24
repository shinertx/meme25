use anyhow::{anyhow, Result};
use once_cell::sync::OnceCell;
use reqwest::Client;
use shared_models::{error::ModelError, SignRequest, SignResponse};
use std::time::Duration;
use tracing::{debug, error};

static CLIENT: OnceCell<Client> = OnceCell::new();

fn get_client() -> Result<&'static Client> {
    CLIENT
        .get_or_try_init(|| {
            Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| ModelError::Network(format!("Failed to create HTTP client: {}", e)))
        })
        .map_err(|e| anyhow!("HTTP client initialization failed: {}", e))
}

pub async fn get_pubkey() -> Result<String> {
    use crate::config::CONFIG;
    debug!("Getting wallet pubkey from signer service");

    let client = get_client()?;
    let response = client
        .get(format!("{}/pubkey", CONFIG.signer_url))
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow!("Signer pubkey request failed: {}", error_text));
    }

    let json: serde_json::Value = response.json().await?;

    json.get("pubkey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow!("Invalid pubkey response format"))
}

pub async fn sign_transaction(transaction_b64: &str) -> Result<String> {
    use crate::config::CONFIG;
    debug!("Sending transaction to signer service");

    let request = SignRequest {
        transaction_b64: transaction_b64.to_string(),
    };

    let client = get_client()?;
    let response = client
        .post(format!("{}/sign", CONFIG.signer_url))
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        error!("Signer service error: {}", error_text);
        return Err(anyhow!("Transaction signing failed: {}", error_text));
    }

    let sign_response: SignResponse = response.json().await?;

    debug!("Transaction signed successfully");
    Ok(sign_response.signed_transaction_b64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_pubkey() {
        // This would require a running signer service for integration tests
        // In CI/CD, you'd mock this or use a test signer
    }
}
