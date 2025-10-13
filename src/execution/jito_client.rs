use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use bincode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use solana_sdk::{
    signature::{Keypair, Signature},
    transaction::Transaction,
};
use std::sync::Arc;
use tracing::{info, warn};

pub struct JitoClient {
    block_engine_url: String,
    auth_keypair: Arc<Keypair>,
    tip_amount: u64,
}

impl JitoClient {
    pub fn new(block_engine_url: String, auth_keypair: Arc<Keypair>) -> Self {
        Self {
            block_engine_url,
            auth_keypair,
            tip_amount: 10_000, // 0.00001 SOL tip
        }
    }

    pub async fn send_bundle(&self, transactions: Vec<Transaction>) -> Result<Signature> {
        // MEV Protection Strategy:
        // 1. Bundle our transaction with a tip
        // 2. Send directly to Jito block builders
        // 3. Bypass public mempool to avoid sandwiching

        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            info!(
                "Paper trading: Would send Jito bundle with {} transactions",
                transactions.len()
            );
            return Ok(Signature::default());
        }

        if transactions.is_empty() {
            return Err(anyhow::anyhow!("Cannot submit empty bundle to Jito"));
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to create Jito HTTP client")?;

        let tip_accounts = self.fetch_tip_accounts(&client).await?;
        let tip_account = tip_accounts
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No Jito tip accounts available"))?;

        let encoded_txs: Vec<String> = transactions
            .iter()
            .map(|tx| {
                let bytes = bincode::serialize(tx)
                    .context("Failed to serialise transaction for Jito bundle")?;
                Ok(BASE64.encode(bytes))
            })
            .collect::<Result<Vec<_>>>()?;

        #[derive(Serialize)]
        struct BundleRequest {
            #[serde(rename = "transactions")]
            txs: Vec<String>,
            #[serde(rename = "tipAccount")]
            tip_account: String,
            #[serde(rename = "tipLamports")]
            tip_lamports: u64,
        }

        #[derive(Debug, Deserialize)]
        struct BundleResponse {
            #[serde(rename = "bundleId")]
            bundle_id: Option<String>,
            status: Option<String>,
        }

        let payload = BundleRequest {
            txs: encoded_txs,
            tip_account,
            tip_lamports: self.tip_amount,
        };

        let url = format!("{}/api/v1/bundles", self.block_engine_url);
        let auth_identity = self.auth_keypair.pubkey().to_string();
        let response = client
            .post(&url)
            .header("x-block-engine-identity", auth_identity)
            .json(&payload)
            .send()
            .await
            .context("Failed to submit bundle to Jito")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jito bundle submission failed: {}", body));
        }

        let bundle_response: BundleResponse = response
            .json()
            .await
            .context("Failed to parse Jito bundle response")?;

        if let Some(status) = bundle_response.status.as_deref() {
            if status != "submitted" && status != "processing" {
                warn!(status, "Jito bundle returned non-success status");
            }
        }

        // Use the first transaction signature as confirmation handle
        let primary_signature = transactions
            .first()
            .and_then(|tx| tx.signatures.first())
            .cloned()
            .unwrap_or_default();

        info!(
            bundle = bundle_response.bundle_id.as_deref().unwrap_or("unknown"),
            sig = %primary_signature,
            "Submitted bundle to Jito"
        );

        Ok(primary_signature)
    }

    async fn fetch_tip_accounts(&self, client: &Client) -> Result<Vec<String>> {
        let url = format!("{}/api/v1/bundles/tip_accounts", self.block_engine_url);
        let response = client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Jito tip accounts")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(status = %response.status(), "Jito tip account request failed: {}", body);
            return Ok(Vec::new());
        }

        let payload: JsonValue = response
            .json()
            .await
            .context("Failed to parse Jito tip accounts response")?;

        let accounts = payload
            .get("data")
            .and_then(|data| data.get("tipAccounts"))
            .or_else(|| payload.get("tipAccounts"))
            .and_then(|value| value.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        Ok(accounts)
    }

    pub fn estimate_tip(&self, priority: Priority) -> u64 {
        match priority {
            Priority::Low => 1_000,       // 0.000001 SOL
            Priority::Medium => 10_000,   // 0.00001 SOL
            Priority::High => 100_000,    // 0.0001 SOL
            Priority::Ultra => 1_000_000, // 0.001 SOL
        }
    }
}

pub enum Priority {
    Low,
    Medium,
    High,
    Ultra,
}
