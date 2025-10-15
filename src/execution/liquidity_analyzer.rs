use anyhow::{Context, Result};
use reqwest::{
    header::{HeaderName, HeaderValue, AUTHORIZATION},
    Client, RequestBuilder,
};
use serde_json::Value as JsonValue;
use tracing::{debug, warn};

fn with_jupiter_headers(builder: RequestBuilder) -> RequestBuilder {
    match std::env::var("JUPITER_API_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            let trimmed = key.trim();
            let header_name = std::env::var("JUPITER_API_KEY_HEADER")
                .ok()
                .and_then(|name| HeaderName::from_bytes(name.as_bytes()).ok())
                .unwrap_or_else(|| HeaderName::from_static("x-api-key"));
            let prefix = std::env::var("JUPITER_API_KEY_PREFIX").unwrap_or_default();
            let include_bearer = std::env::var("JUPITER_INCLUDE_BEARER")
                .map(|v| v != "false")
                .unwrap_or(true);
            let header_value_str = format!("{}{}", prefix, trimmed);

            let mut builder = match HeaderValue::from_str(&header_value_str) {
                Ok(value) => builder.header(header_name, value),
                Err(_) => builder,
            };

            if include_bearer {
                if let Ok(auth_value) =
                    HeaderValue::from_str(&format!("Bearer {}", trimmed))
                {
                    builder = builder.header(AUTHORIZATION, auth_value);
                }
            }

            builder
        }
        _ => builder,
    }
}

pub struct LiquidityAnalyzer {
    min_liquidity_usd: f64,
    max_price_impact: f64,
    max_slippage_bps: u16,
}

impl LiquidityAnalyzer {
    pub fn new() -> Self {
        Self {
            min_liquidity_usd: 10_000.0, // $10k minimum liquidity
            max_price_impact: 0.02,      // 2% max price impact
            max_slippage_bps: 100,       // 1% max slippage
        }
    }

    pub async fn check_tradeable(&self, token: &str, amount_usd: f64) -> Result<TradingViability> {
        // Get liquidity data from multiple sources
        let pool_data = self.get_pool_data(token).await?;

        // Check minimum liquidity
        if pool_data.liquidity_usd < self.min_liquidity_usd {
            return Ok(TradingViability::NotViable {
                reason: format!(
                    "Insufficient liquidity: ${:.2} < ${:.2}",
                    pool_data.liquidity_usd, self.min_liquidity_usd
                ),
            });
        }

        // Calculate price impact
        let price_impact =
            self.calculate_price_impact(amount_usd, pool_data.liquidity_usd, pool_data.fee_bps);

        if price_impact > self.max_price_impact {
            return Ok(TradingViability::NotViable {
                reason: format!(
                    "Price impact too high: {:.2}% > {:.2}%",
                    price_impact * 100.0,
                    self.max_price_impact * 100.0
                ),
            });
        }

        // Check if token is rugpull risk
        if pool_data.lp_burned_percent < 90.0 {
            return Ok(TradingViability::Risky {
                warnings: vec![format!(
                    "LP not fully burned: {:.1}%",
                    pool_data.lp_burned_percent
                )],
                suggested_size_reduction: 0.5, // Trade 50% of intended size
            });
        }

        Ok(TradingViability::Viable {
            expected_slippage_bps: (price_impact * 10000.0) as u16,
            available_liquidity_usd: pool_data.liquidity_usd,
        })
    }

    async fn get_pool_data(&self, token: &str) -> Result<PoolData> {
        if let Ok(api_key) = std::env::var("BIRDEYE_API_KEY") {
            if !api_key.is_empty() && api_key.to_lowercase() != "none" {
                if let Some(pool) = self.fetch_birdeye_pool(token, &api_key).await? {
                    return Ok(pool);
                }
            }
        }

        if let Some(pool) = self.fetch_dexscreener_pool(token).await? {
            return Ok(pool);
        }

        if let Some(pool) = self.fetch_jupiter_pool(token).await? {
            return Ok(pool);
        }

        // Fallback values suitable for paper trading
        Ok(PoolData {
            liquidity_usd: 25_000.0,
            fee_bps: 30,
            lp_burned_percent: 100.0,
        })
    }

    async fn fetch_dexscreener_pool(&self, token: &str) -> Result<Option<PoolData>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to build DexScreener HTTP client")?;

        let url = format!(
            "https://api.dexscreener.com/latest/dex/tokens/{}",
            token
        );

        let response = client
            .get(&url)
            .send()
            .await
            .context("DexScreener request failed")?;

        if !response.status().is_success() {
            warn!(
                status = %response.status(),
                "DexScreener request for {} returned non-success",
                token
            );
            return Ok(None);
        }

        let payload: JsonValue = response
            .json()
            .await
            .context("Failed to parse DexScreener response")?;

        let pairs = payload.get("pairs").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if pairs.is_empty() {
            return Ok(None);
        }

        // Take the highest liquidity pair on Solana
        let mut best_liquidity = 0.0f64;
        let mut best_fee_bps = 30u16;
        for pair in pairs {
            if pair.get("chainId").and_then(|v| v.as_str()) != Some("solana") {
                continue;
            }
            let liq = pair
                .get("liquidityUsd")
                .or_else(|| pair.get("fdv"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            if liq > best_liquidity {
                best_liquidity = liq;
                best_fee_bps = pair
                    .get("lpFeeBps")
                    .or_else(|| pair.get("feeBps"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(30) as u16;
            }
        }

        if best_liquidity <= 0.0 {
            return Ok(None);
        }

        Ok(Some(PoolData {
            liquidity_usd: best_liquidity,
            fee_bps: best_fee_bps,
            lp_burned_percent: 100.0,
        }))
    }

    async fn fetch_birdeye_pool(&self, token: &str, api_key: &str) -> Result<Option<PoolData>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to build Birdeye HTTP client")?;

        let url = format!(
            "https://public-api.birdeye.so/defi/token_overview?address={}",
            token
        );

        let response = client
            .get(&url)
            .header("X-API-KEY", api_key)
            .send()
            .await
            .context("Birdeye request failed")?;

        if !response.status().is_success() {
            warn!(
                status = %response.status(),
                "Birdeye request for {} returned non-success",
                token
            );
            return Ok(None);
        }

        let payload: JsonValue = response
            .json()
            .await
            .context("Failed to parse Birdeye response")?;

        let data = match payload.get("data") {
            Some(value) => value,
            None => return Ok(None),
        };

        let liquidity = data
            .get("liquidity")
            .and_then(|v| v.as_f64())
            .or_else(|| data.get("liquidity_usd").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        if liquidity <= 0.0 {
            return Ok(None);
        }

        let fee_bps = data.get("fee_bps").and_then(|v| v.as_u64()).unwrap_or(30) as u16;

        let lp_burned_percent = data
            .get("lp_burn_percent")
            .or_else(|| data.get("lpBurnPercent"))
            .or_else(|| data.get("lp_locked"))
            .and_then(|v| v.as_f64())
            .unwrap_or(100.0);

        debug!(
            token = token,
            liquidity = liquidity,
            "Birdeye liquidity snapshot retrieved"
        );

        Ok(Some(PoolData {
            liquidity_usd: liquidity,
            fee_bps,
            lp_burned_percent,
        }))
    }

    async fn fetch_jupiter_pool(&self, token: &str) -> Result<Option<PoolData>> {
        let api_url = std::env::var("JUPITER_BASE_URL")
            .unwrap_or_else(|_| "https://api.jup.ag/swap/v1".to_string());

        let usdc_mint = std::env::var("USDC_MINT")
            .unwrap_or_else(|_| "EPjFWdd5AufqSSqeM2qZ8d8bP8XPhYet6GtDq3z31g".to_string());

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .context("Failed to build Jupiter HTTP client")?;

        let amount_usdc = 100_000_000u64; // 100 USDC with 6 decimals

        let request = client
            .get(format!("{}/quote", api_url))
            .query(&[
                ("inputMint", usdc_mint.as_str()),
                ("outputMint", token),
                ("amount", &amount_usdc.to_string()),
                ("slippageBps", "100"),
                ("onlyDirectRoutes", "false"),
                ("asLegacyTransaction", "false"),
            ]);

        let response = with_jupiter_headers(request)
            .send()
            .await
            .context("Jupiter quote request failed")?;

        if !response.status().is_success() {
            warn!(
                status = %response.status(),
                "Jupiter quote for {} returned non-success",
                token
            );
            return Ok(None);
        }

        let payload: JsonValue = response
            .json()
            .await
            .context("Failed to parse Jupiter response")?;

        let impact_pct = payload
            .get("priceImpactPct")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if impact_pct <= 0.0 {
            return Ok(None);
        }

        let fee_bps = payload
            .get("routePlan")
            .and_then(|plan| plan.get(0))
            .and_then(|route| route.get("marketInfos"))
            .and_then(|infos| infos.get(0))
            .and_then(|info| info.get("lpFee"))
            .and_then(|fee| fee.get("feeBps"))
            .and_then(|v| v.as_u64())
            .unwrap_or(30) as u16;

        let trade_size_usd = amount_usdc as f64 / 1_000_000.0;
        let liquidity_estimate = trade_size_usd / (impact_pct / 100.0);

        debug!(
            token = token,
            liquidity = liquidity_estimate,
            "Jupiter liquidity estimate computed"
        );

        Ok(Some(PoolData {
            liquidity_usd: liquidity_estimate,
            fee_bps,
            lp_burned_percent: 100.0,
        }))
    }

    fn calculate_price_impact(&self, trade_size: f64, liquidity: f64, fee_bps: u16) -> f64 {
        // Simplified constant product AMM formula
        let fee_multiplier = 1.0 - (fee_bps as f64 / 10000.0);
        let size_ratio = trade_size / liquidity;

        // Price impact approximation
        size_ratio / (1.0 - size_ratio) * fee_multiplier
    }
}

pub enum TradingViability {
    Viable {
        expected_slippage_bps: u16,
        available_liquidity_usd: f64,
    },
    Risky {
        warnings: Vec<String>,
        suggested_size_reduction: f64,
    },
    NotViable {
        reason: String,
    },
}

struct PoolData {
    liquidity_usd: f64,
    fee_bps: u16,
    lp_burned_percent: f64,
}
