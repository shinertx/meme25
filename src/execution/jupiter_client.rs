use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Keypair};
use std::str::FromStr;
use tracing::info;

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    pub slippage_bps: u16,
    pub only_direct_routes: bool,
    pub as_legacy_transaction: bool,
}

#[derive(Debug, Deserialize)]
pub struct QuoteResponse {
    pub in_amount: String,
    pub out_amount: String,
    pub price_impact_pct: f64,
    pub route_plan: Vec<RoutePlan>,
}

#[derive(Debug, Deserialize)]
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
        
        let response = self.client
            .get(&url)
            .query(&[
                ("inputMint", &request.input_mint),
                ("outputMint", &request.output_mint),
                ("amount", &request.amount.to_string()),
                ("slippageBps", &request.slippage_bps.to_string()),
                ("onlyDirectRoutes", &request.only_direct_routes.to_string()),
                ("asLegacyTransaction", &request.as_legacy_transaction.to_string()),
            ])
            .send()
            .await
            .context("Failed to get quote from Jupiter")?;
            
        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("Jupiter API error: {}", error_text));
        }
        
        response.json::<QuoteResponse>()
            .await
            .context("Failed to parse Jupiter quote response")
    }
    
    pub async fn swap(&self, quote: QuoteResponse) -> Result<String> {
        // In production, this would:
        // 1. Build the transaction from the quote
        // 2. Sign with self.keypair
        // 3. Send via Jito for MEV protection
        
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            info!("Paper trading: Would execute swap for {} -> {}", 
                quote.in_amount, quote.out_amount);
            return Ok("paper_trade_tx_".to_string() + &uuid::Uuid::new_v4().to_string());
        }
        
        // TODO: Implement real swap execution
        Err(anyhow::anyhow!("Real trading not yet implemented"))
    }
}
