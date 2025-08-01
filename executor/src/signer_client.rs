use crate::config::CONFIG;
use anyhow::{anyhow, Result};
use reqwest::Client;
use shared_models::{SignRequest, SignResponse};
use std::time::Duration;
use tracing::{debug, error};

static CLIENT: once_cell::sync::Lazy<Client> = once_cell::sync::Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
});

pub async fn get_pubkey() -> Result<String> {
    debug!("Getting wallet pubkey from signer service");
    
    let response = CLIENT
        .get(&format!("{}/pubkey", CONFIG.signer_url))
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
    debug!("Sending transaction to signer service");
    
    let request = SignRequest {
        transaction_b64: transaction_b64.to_string(),
    };
    
    let response = CLIENT
        .post(&format!("{}/sign", CONFIG.signer_url))
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
