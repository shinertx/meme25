use anyhow::{Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::VersionedTransaction,
};
use std::str::FromStr;
use tracing::info;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    pub slippage_bps: u16,
    pub only_direct_routes: bool,
    pub as_legacy_transaction: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteResponse {
    pub in_amount: String,
    pub out_amount: String,
    pub price_impact_pct: f64,
    pub route_plan: Vec<RoutePlan>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutePlan {
    pub amm_key: String,
    pub label: String,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub fee_mint: String,
    pub fee_pct: f64,
}

pub struct JupiterClient {
    client: Client,
    api_url: String,
    keypair: Keypair,
}

impl JupiterClient {
    pub fn new(api_url: String, keypair: Keypair) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            api_url,
            keypair,
        })
    }

    pub async fn get_quote(&self, request: &QuoteRequest) -> Result<QuoteResponse> {
        let url = format!("{}/quote", self.api_url);

        let response = self
            .client
            .get(&url)
            .query(&[
                ("inputMint", &request.input_mint),
                ("outputMint", &request.output_mint),
                ("amount", &request.amount.to_string()),
                ("slippageBps", &request.slippage_bps.to_string()),
                ("onlyDirectRoutes", &request.only_direct_routes.to_string()),
                (
                    "asLegacyTransaction",
                    &request.as_legacy_transaction.to_string(),
                ),
            ])
            .send()
            .await
            .context("Failed to get quote from Jupiter")?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Jupiter API error: {}", error_text));
        }

        response
            .json::<QuoteResponse>()
            .await
            .context("Failed to parse Jupiter quote response")
    }

    pub async fn swap(&self, quote: QuoteResponse) -> Result<String> {
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            info!(
                "Paper trading: Would execute swap for {} -> {}",
                quote.in_amount, quote.out_amount
            );
            return Ok(format!("paper_trade_tx_{}", Uuid::new_v4()));
        }

        let swap_url = format!("{}/swap", self.api_url);
        let swap_payload = serde_json::json!({
            "quoteResponse": quote,
            "userPublicKey": self.keypair.pubkey().to_string(),
            "wrapAndUnwrapSol": true,
        });

        let response = self
            .client
            .post(&swap_url)
            .json(&swap_payload)
            .send()
            .await
            .context("Failed to request Jupiter swap transaction")?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Jupiter swap error: {}", body));
        }

        let response_json: serde_json::Value = response
            .json()
            .await
            .context("Failed to parse Jupiter swap response")?;

        let swap_tx_b64 = response_json
            .get("swapTransaction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Swap response missing transaction payload"))?;

        let tx_bytes = BASE64
            .decode(swap_tx_b64.as_bytes())
            .context("Failed to decode swap transaction")?;

        let mut transaction: VersionedTransaction =
            bincode::deserialize(&tx_bytes).context("Failed to deserialize swap transaction")?;

        transaction.sign(&[&self.keypair]);

        let signed_tx = BASE64.encode(
            bincode::serialize(&transaction).context("Failed to serialize signed transaction")?,
        );

        let rpc_url =
            std::env::var("SOLANA_RPC_URL").context("SOLANA_RPC_URL not set for swap execution")?;

        let rpc_client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("Failed to build Solana RPC client")?;

        let rpc_payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": [
                signed_tx,
                {
                    "encoding": "base64",
                    "skipPreflight": false,
                    "maxRetries": 3
                }
            ]
        });

        let rpc_response = rpc_client
            .post(&rpc_url)
            .json(&rpc_payload)
            .send()
            .await
            .context("Failed to submit transaction to Solana RPC")?;

        let rpc_json: serde_json::Value = rpc_response
            .json()
            .await
            .context("Failed to parse Solana RPC response")?;

        if let Some(error) = rpc_json.get("error") {
            return Err(anyhow::anyhow!("Solana RPC error: {}", error));
        }

        let signature = rpc_json
            .get("result")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("RPC response missing transaction signature"))?
            .to_string();

        info!(%signature, "Executed live swap via Jupiter");

        Ok(signature)
    }
}
