use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct JitoBundle {
    pub transactions: Vec<String>,
    pub tip_lamports: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JitoBundleResponse {
    pub bundle_id: String,
    pub status: String,
}

pub struct JitoClient {
    client: Client,
    base_url: String,
    auth_keypair: Option<String>,
}

impl JitoClient {
    pub fn new(base_url: String, auth_keypair: Option<String>) -> Self {
        Self {
            client: Client::new(),
            base_url,
            auth_keypair,
        }
    }

    pub async fn submit_bundle(&self, bundle: JitoBundle) -> Result<JitoBundleResponse> {
        // For paper trading mode, just simulate the response
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            tracing::info!("📦 PAPER MODE: Simulating Jito bundle submission");
            return Ok(JitoBundleResponse {
                bundle_id: format!("paper_bundle_{}", uuid::Uuid::new_v4()),
                status: "simulated".to_string(),
            });
        }

        let url = format!("{}/bundles", self.base_url);

        let mut request = self.client.post(&url);
        if let Some(auth) = &self.auth_keypair {
            request = request.header("x-block-engine-identity", auth);
        }

        let response = request.json(&bundle).send().await?;

        if response.status().is_success() {
            let bundle_response: JitoBundleResponse = response.json().await?;
            tracing::info!("✅ Jito bundle submitted: {}", bundle_response.bundle_id);
            Ok(bundle_response)
        } else {
            let error_text = response.text().await?;
            anyhow::bail!("Jito bundle submission failed: {}", error_text);
        }
    }

    pub async fn get_bundle_status(&self, bundle_id: &str) -> Result<String> {
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            return Ok("simulated_confirmed".to_string());
        }

        let url = format!("{}/bundles/{}", self.base_url, bundle_id);

        let mut request = self.client.get(&url);
        if let Some(auth) = &self.auth_keypair {
            request = request.header("x-block-engine-identity", auth);
        }

        let response = request.send().await?;

        if response.status().is_success() {
            let status_response: serde_json::Value = response.json().await?;
            Ok(status_response["status"]
                .as_str()
                .unwrap_or("unknown")
                .to_string())
        } else {
            anyhow::bail!("Failed to get bundle status");
        }
    }
}
