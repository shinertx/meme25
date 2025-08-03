use anyhow::Result;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use shared_models::error::ModelError;
use tracing::{info, warn, debug};

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteRequest {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    pub amount: u64,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteResponse {
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapRequest {
    pub quote: QuoteResponse,
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    pub wrap_and_unwrap_sol: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String,
}

#[derive(Debug, Clone)]
pub struct LiquidityAnalysis {
    pub available_liquidity_usd: f64,
    pub price_impact_pct: f64,
    pub slippage_tolerance_bps: u16,
    pub estimated_slippage_bps: u16,
    pub is_liquid_enough: bool,
    pub route_quality: RouteQuality,
}

#[derive(Debug, Clone)]
pub enum RouteQuality {
    Excellent,  // Direct route, minimal hops
    Good,       // 2-3 hops, acceptable slippage
    Poor,       // Many hops, high slippage
    Unviable,   // Excessive slippage or no route
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenInfoRequest {
    pub token_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenInfo {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub daily_volume: Option<f64>,
}

pub struct JupiterClient {
    client: Client,
    base_url: String,
}

impl JupiterClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }

    pub async fn get_quote(&self, request: QuoteRequest) -> Result<QuoteResponse> {
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            tracing::info!("📊 PAPER MODE: Simulating Jupiter quote");
            return Ok(QuoteResponse {
                out_amount: "1000000".to_string(), // Mock 1 USDC
                price_impact_pct: "0.1".to_string(),
                route_plan: vec![],
            });
        }

        let url = format!("{}/quote", self.base_url);
        
        let response = self.client
            .get(&url)
            .query(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let quote: QuoteResponse = response.json().await?;
            tracing::debug!("📊 Jupiter quote received: {} output amount", quote.out_amount);
            Ok(quote)
        } else {
            let error_text = response.text().await?;
            anyhow::bail!("Jupiter quote request failed: {}", error_text);
        }
    }

    pub async fn get_swap(&self, request: SwapRequest) -> Result<SwapResponse> {
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            tracing::info!("🔄 PAPER MODE: Simulating Jupiter swap transaction");
            return Ok(SwapResponse {
                swap_transaction: "paper_transaction_base64".to_string(),
            });
        }

        let url = format!("{}/swap", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().is_success() {
            let swap: SwapResponse = response.json().await?;
            tracing::info!("🔄 Jupiter swap transaction generated");
            Ok(swap)
        } else {
            let error_text = response.text().await?;
            anyhow::bail!("Jupiter swap request failed: {}", error_text);
        }
    }

    pub async fn get_price(&self, input_mint: &str, output_mint: &str) -> Result<f64> {
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            return Ok(0.001234); // Mock price
        }

        let quote_request = QuoteRequest {
            input_mint: input_mint.to_string(),
            output_mint: output_mint.to_string(),
            amount: 1_000_000, // 1 token with 6 decimals
            slippage_bps: 50,
        };

        let quote = self.get_quote(quote_request).await?;
        let price = quote.out_amount.parse::<f64>()? / 1_000_000.0;
        Ok(price)
    }

    /// Analyze liquidity for a potential trade (Fix #6: Liquidity & Slippage Analysis)
    pub async fn analyze_liquidity(&self, 
        input_mint: &str, 
        output_mint: &str, 
        amount_usd: f64,
        max_slippage_bps: u16
    ) -> Result<LiquidityAnalysis> {
        use tracing::{info, warn, debug};
        
        debug!("Analyzing liquidity for ${} trade: {} -> {}", amount_usd, input_mint, output_mint);

        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            info!("📊 PAPER MODE: Simulating liquidity analysis");
            return Ok(LiquidityAnalysis {
                available_liquidity_usd: amount_usd * 10.0, // Assume 10x liquidity available
                price_impact_pct: if amount_usd > 1000.0 { 0.5 } else { 0.1 },
                slippage_tolerance_bps: max_slippage_bps,
                estimated_slippage_bps: if amount_usd > 1000.0 { 25 } else { 10 },
                is_liquid_enough: amount_usd < 5000.0,
                route_quality: if amount_usd < 1000.0 { RouteQuality::Excellent } else { RouteQuality::Good },
            });
        }

        // Convert USD to token amount (assuming 6 decimals)
        let token_amount = (amount_usd * 1_000_000.0) as u64;

        // Test multiple quote sizes to gauge liquidity depth
        let quote_sizes = vec![
            token_amount / 10,  // 10% of trade
            token_amount / 2,   // 50% of trade  
            token_amount,       // Full trade
            token_amount * 2,   // 2x trade size
        ];

        let mut price_impacts = Vec::new();
        let mut successful_quotes = 0;

        for size in quote_sizes {
            let quote_request = QuoteRequest {
                input_mint: input_mint.to_string(),
                output_mint: output_mint.to_string(),
                amount: size,
                slippage_bps: max_slippage_bps,
            };

            match self.get_quote(quote_request).await {
                Ok(quote) => {
                    let impact: f64 = quote.price_impact_pct.parse().unwrap_or(0.0);
                    price_impacts.push(impact);
                    successful_quotes += 1;
                }
                Err(e) => {
                    warn!("Quote failed for size {}: {}", size, e);
                    break;
                }
            }
        }

        if successful_quotes == 0 {
            return Ok(LiquidityAnalysis {
                available_liquidity_usd: 0.0,
                price_impact_pct: 100.0,
                slippage_tolerance_bps: max_slippage_bps,
                estimated_slippage_bps: 9999,
                is_liquid_enough: false,
                route_quality: RouteQuality::Unviable,
            });
        }

        // Analyze the results
        let avg_price_impact = price_impacts.iter().sum::<f64>() / price_impacts.len() as f64;
        let max_price_impact = price_impacts.iter().fold(0.0f64, |a, &b| a.max(b));
        
        // Estimate available liquidity based on when price impact becomes excessive
        let available_liquidity_usd = if max_price_impact < 1.0 {
            amount_usd * 5.0 // Good liquidity
        } else if max_price_impact < 3.0 {
            amount_usd * 2.0 // Moderate liquidity
        } else {
            amount_usd * 0.5 // Poor liquidity
        };

        // Determine route quality
        let quality = match successful_quotes {
            4..=10 if max_price_impact < 0.5 => RouteQuality::Excellent,
            3 if max_price_impact < 2.0 => RouteQuality::Good,
            2 if max_price_impact < 5.0 => RouteQuality::Poor,
            _ => RouteQuality::Poor,
        };

        // Estimate actual slippage (typically higher than price impact)
        let estimated_slippage_bps = ((avg_price_impact * 100.0) + 10.0) as u16;
        
        let analysis = LiquidityAnalysis {
            available_liquidity_usd,
            price_impact_pct: avg_price_impact,
            slippage_tolerance_bps: max_slippage_bps,
            estimated_slippage_bps,
            is_liquid_enough: estimated_slippage_bps <= max_slippage_bps && successful_quotes >= 2,
            route_quality: quality,
        };

        info!("📊 Liquidity analysis: ${:.0} available, {:.2}% impact, {} quality", 
              analysis.available_liquidity_usd, 
              analysis.price_impact_pct,
              match analysis.route_quality {
                  RouteQuality::Excellent => "excellent",
                  RouteQuality::Good => "good", 
                  RouteQuality::Poor => "poor",
                  RouteQuality::Unviable => "unviable",
              });

        Ok(analysis)
    }

    /// Check if a trade is viable given liquidity constraints
    pub async fn is_trade_viable(&self,
        input_mint: &str,
        output_mint: &str, 
        amount_usd: f64,
        max_slippage_bps: u16,
        min_liquidity_multiple: f64
    ) -> Result<bool> {
        let analysis = self.analyze_liquidity(input_mint, output_mint, amount_usd, max_slippage_bps).await?;
        
        let liquidity_ok = analysis.available_liquidity_usd >= amount_usd * min_liquidity_multiple;
        let slippage_ok = analysis.estimated_slippage_bps <= max_slippage_bps;
        let route_ok = !matches!(analysis.route_quality, RouteQuality::Unviable);
        
        Ok(liquidity_ok && slippage_ok && route_ok)
    }
}
