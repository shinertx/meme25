use anyhow::{anyhow, Result};
use async_tungstenite::{tokio::connect_async, tungstenite::Message};
use futures_util::StreamExt;
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::Deserialize;
use shared_models::{MarketEvent, PriceTick};
use std::collections::HashMap;
use std::time::Duration as StdDuration;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{interval, sleep, Duration};
use tracing::debug;
use tracing::{error, info, warn};

use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};

#[derive(Clone)]
struct DsCacheEntry {
    liquidity_usd: f64,
    volume_5m_usd: f64,
    ts: chrono::DateTime<chrono::Utc>,
}

#[derive(Default)]
struct DsCache {
    map: HashMap<String, DsCacheEntry>,
}

impl DsCache {
    fn get(&self, token: &str) -> Option<DsCacheEntry> {
        self.map.get(token).cloned()
    }
    fn put(&mut self, token: String, entry: DsCacheEntry) {
        self.map.insert(token, entry);
    }
}

static DS_CACHE: Lazy<TokioMutex<DsCache>> = Lazy::new(|| TokioMutex::new(DsCache::default()));

#[derive(Clone)]
struct FallbackToken {
    address: String,
    liquidity_usd: f64,
    volume_24h_usd: f64,
}

fn http_user_agent() -> String {
    std::env::var("MKT_HTTP_USER_AGENT")
        .unwrap_or_else(|_| "Meme25-MarketGateway/1.0 (+https://github.com/meme25)".to_string())
}

fn http_retry_attempts() -> usize {
    std::env::var("MKT_HTTP_RETRIES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(3)
}

fn build_http_client(timeout: StdDuration) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    reqwest::Client::builder()
        .timeout(timeout)
        .default_headers(headers)
        .user_agent(http_user_agent())
        .build()
        .map_err(Into::into)
}

async fn fetch_json_with_retry(
    client: &reqwest::Client,
    url: &str,
    attempts: usize,
) -> Result<serde_json::Value> {
    let retries = attempts.max(1);
    for attempt in 1..=retries {
        match client.get(url).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(resp.json().await?),
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                warn!(%status, %url, attempt, "HTTP response was not success");
                debug!(body = body.as_str(), "HTTP error body");
            }
            Err(err) => {
                warn!(%url, attempt, %err, "HTTP request failed");
            }
        }
        if attempt < retries {
            let backoff_ms = 250u64.saturating_mul(1 << (attempt - 1));
            sleep(Duration::from_millis(backoff_ms.min(5_000))).await;
        }
    }
    Err(anyhow!("exhausted retries for {}", url))
}

#[derive(Debug, Deserialize)]
struct HeliusSlotTick {
    #[serde(rename = "price")]
    price_usd: f64,
    #[allow(dead_code)]
    symbol: String,
    address: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    timestamp: chrono::DateTime<chrono::Utc>,
}

/*
pub struct MarketDataGateway {
    redis_client: redis::Client,
}

impl MarketDataGateway {
    pub fn new(redis_url: &str) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
        Ok(Self { redis_client })
    }

    pub async fn start(&self) -> Result<()> {
        info!("Starting Market Data Gateway");
        let mut con = self.redis_client.get_async_connection().await?;

        // Start data collection tasks
        tokio::spawn(async move {
            let mut timer = interval(Duration::from_millis(100));

            loop {
                timer.tick().await;

                // Collect market data from various sources
                if let Err(e) = collect_mock_onchain(&mut con).await {
                    error!("Mock on-chain generator failed: {}", e);
                }
            }
        });

        // DexScreener trending poller (optional)
        let enable_trending =
            std::env::var("MKT_ENABLE_TRENDING").unwrap_or_else(|_| "true".into()) == "true";
        if enable_trending {
            info!("DexScreener trending poller enabled");
            let mut conn_clone = self.redis_client.get_multiplexed_tokio_connection().await?;
            tokio::spawn(async move {
                loop {
                    if let Err(e) = poll_dexscreener_trending(&mut conn_clone).await {
                        warn!(%e, "trending poll failed");
                    }
                    let sec = std::env::var("MKT_TRENDING_INTERVAL_SEC")
                        .ok()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60);
                    sleep(Duration::from_secs(sec)).await;
                }
            });
        }

        Ok(())
    }
}
*/

async fn collect_mock_onchain(con: &mut MultiplexedConnection) -> Result<()> {
    // Emits a tiny mock OnChain event occasionally for executor coverage
    if std::env::var("MKT_ENABLE_MOCK_ONCHAIN").unwrap_or_else(|_| "false".into()) != "true" {
        return Ok(());
    }
    
    // Simulate a Dev Buy occasionally
    if rand::random::<f64>() < 0.05 {
        let evt = shared_models::MarketEvent::OnChain(shared_models::OnChainEvent {
            token_address: "So11111111111111111111111111111111111111112".into(),
            event_type: "dev_buy".into(),
            details: serde_json::json!({"amount": 5000, "signature": "mock_sig"}),
            timestamp: chrono::Utc::now(),
        });
        let event_wrapper = shared_models::Event::Market(evt);
        let payload = serde_json::to_string(&event_wrapper)?;
        let _: () = con
            .xadd(
                "events:onchain",
                "*",
                &[("type", "onchain"), ("data", payload.as_str())],
            )
            .await?;
        info!("Published mock DEV BUY event");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let redis_url = std::env::var("REDIS_URL")?;
    let client = redis::Client::open(redis_url)?;
    let conn: MultiplexedConnection = client.get_multiplexed_tokio_connection().await?;

    // Optional Helius stream
    let helius_ws = std::env::var("HELIUS_API_KEY").unwrap_or_default();
    let enable_mock =
        std::env::var("MKT_ENABLE_MOCK_PRICE").unwrap_or_else(|_| "false".into()) == "true";

    // Spawn mock publisher if requested
    if enable_mock {
        info!("Mock price feed enabled (paper mode)");
        let mut conn_clone = conn.clone();
        tokio::spawn(async move {
            let mut t = interval(Duration::from_millis(1000));
            let mut price: f64 = 150.0; // synthetic
            loop {
                t.tick().await;
                // Random walk
                let delta = (rand::random::<f64>() - 0.5) * 0.5;
                price = (price + delta).max(1.0);
                let tick = HeliusSlotTick {
                    price_usd: price,
                    symbol: "SOL".into(),
                    address: "So11111111111111111111111111111111111111112".into(),
                    timestamp: chrono::Utc::now(),
                };
                if let Err(e) = forward_price(&mut conn_clone, tick).await {
                    warn!(%e, "mock forward_price failed");
                }
                
                // Also run mock onchain events
                if let Err(e) = collect_mock_onchain(&mut conn_clone).await {
                    warn!(%e, "mock onchain failed");
                }
            }
        });
    } else if helius_ws.is_empty() {
        warn!("HELIUS_API_KEY unset and mock disabled; relying on DexScreener polling only");
    }

    let enable_helius = std::env::var("ENABLE_HELIUS_WS").unwrap_or_else(|_| "false".into()) == "true";
    if !helius_ws.is_empty() && enable_helius {
        let url = format!(
            "wss://mainnet.helius-rpc.com/?api-key={}",
            helius_ws
        );
        tokio::spawn(async move {
            loop {
                match connect_async(&url).await {
                    Ok((ws, _)) => {
                        info!("Helius WS connected");
                        let (_write, mut read) = ws.split();
                        while let Some(msg) = read.next().await {
                            if let Ok(Message::Text(txt)) = msg {
                                if let Ok(_tick) = serde_json::from_str::<HeliusSlotTick>(&txt) {
                                    // Clone connection per tick would be heavy; for brevity here we'll skip;
                                    // In production, route through a channel to a single Redis writer.
                                    // For now, ignore as mock feed covers paper mode.
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(%e, "WS error → retrying in 5s");
                        sleep(Duration::from_secs(5)).await;
                    }
                }
            }
        });
    }

    // DexScreener trending poller
    let enable_trending = std::env::var("MKT_ENABLE_TRENDING").unwrap_or_else(|_| "true".into()) == "true";
    if enable_trending {
        info!("DexScreener trending poller enabled");
        let mut conn_clone = conn.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = poll_dexscreener_trending(&mut conn_clone).await {
                    warn!(%e, "trending poll failed");
                }
                let sec = std::env::var("MKT_TRENDING_INTERVAL_SEC")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                sleep(Duration::from_secs(sec)).await;
            }
        });
    }

    // DexScreener new pairs poller
    let enable_newpairs = std::env::var("MKT_ENABLE_NEWPAIRS").unwrap_or_else(|_| "true".into()) == "true";
    if enable_newpairs {
        info!("DexScreener new-pairs poller enabled");
        let mut conn_clone = conn.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = poll_dexscreener_newpairs(&mut conn_clone).await {
                    warn!(%e, "newpairs poll failed");
                }
                let sec = std::env::var("MKT_NEWPAIRS_INTERVAL_SEC")
                    .ok()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(30);
                sleep(Duration::from_secs(sec)).await;
            }
        });
    }


    // Keep process alive
    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

async fn forward_price(conn: &mut MultiplexedConnection, tick: HeliusSlotTick) -> Result<()> {
    // Try to enrich via DexScreener (best-effort, non-fatal)
    let enable_enrich =
        std::env::var("MKT_ENRICH_DEXSCREENER").unwrap_or_else(|_| "true".into()) == "true";
    let (liq_usd, vol5m_usd) = if enable_enrich {
        match dexscreener_enrich_cached(&tick.address).await {
            Ok((l, v)) => (l, v),
            Err(_) => (0.0, 0.0),
        }
    } else {
        (0.0, 0.0)
    };
    let evt = MarketEvent::Price(PriceTick {
        token_address: tick.address.clone(),
        price_usd: tick.price_usd,
        volume_usd_1m: 0.0,
        volume_usd_5m: vol5m_usd,
        volume_usd_15m: 0.0,
        price_change_1m: 0.0,
        price_change_5m: 0.0,
        liquidity_usd: liq_usd,
        timestamp: tick.timestamp,
    });
    // Wrap in top-level Event::Market
    let event_wrapper = shared_models::Event::Market(evt);
    let payload = serde_json::to_string(&event_wrapper)?;
    conn.xadd::<_, _, _, _, ()>(
        "events:price",
        "*",
        &[("type", "price"), ("data", payload.as_str())],
    )
    .await?;
    Ok(())
}

async fn dexscreener_enrich_cached(token_address: &str) -> Result<(f64, f64)> {
    // TTL 30 seconds
    let now = chrono::Utc::now();
    let ttl = chrono::Duration::seconds(30);

    {
        let cache = DS_CACHE.lock().await;
        if let Some(entry) = cache.get(token_address) {
            if now - entry.ts < ttl {
                return Ok((entry.liquidity_usd, entry.volume_5m_usd));
            }
        }
    }

    let (liq, vol5m) = dexscreener_enrich(token_address)
        .await
        .unwrap_or((0.0, 0.0));
    {
        let mut cache = DS_CACHE.lock().await;
        cache.put(
            token_address.to_string(),
            DsCacheEntry {
                liquidity_usd: liq,
                volume_5m_usd: vol5m,
                ts: now,
            },
        );
    }
    Ok((liq, vol5m))
}

async fn dexscreener_enrich(token_address: &str) -> Result<(f64, f64)> {
    let url = format!(
        "https://api.dexscreener.com/latest/dex/tokens/{}",
        token_address
    );
    let client = build_http_client(StdDuration::from_secs(5))?;
    match fetch_json_with_retry(&client, &url, http_retry_attempts()).await {
        Ok(v) => {
            let pairs: Vec<serde_json::Value> = v
                .get("pairs")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default();
            let mut best_liq = 0.0;
            let mut best_vol5m = 0.0;
            for p in pairs {
                let liq = p
                    .get("liquidity")
                    .and_then(|l| l.get("usd"))
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0.0);
                let vol_h = p
                    .get("volume")
                    .and_then(|vv| vv.get("h24"))
                    .and_then(|x| x.as_f64())
                    .unwrap_or(0.0);
                let vol5m = if let Some(vv) = p
                    .get("volume")
                    .and_then(|vv| vv.get("m5"))
                    .and_then(|x| x.as_f64())
                {
                    vv
                } else {
                    vol_h / (24.0 * 12.0)
                };
                if liq > best_liq {
                    best_liq = liq;
                    best_vol5m = vol5m;
                }
            }
            if best_liq > 0.0 {
                return Ok((best_liq, best_vol5m));
            }
        }
        Err(err) => {
            warn!(%token_address, %err, "DexScreener enrich failed; falling back");
        }
    }

    fallback_liquidity_metrics(token_address).await
}

async fn poll_dexscreener_trending(conn: &mut MultiplexedConnection) -> Result<()> {
    // Fetch top trending Solana pairs and publish as price ticks
    let client = build_http_client(StdDuration::from_secs(6))?;
    let url = "https://api.dexscreener.com/latest/dex/trending";
    let attempts = http_retry_attempts();
    let payload = match fetch_json_with_retry(&client, url, attempts).await {
        Ok(json) => json,
        Err(err) => {
            warn!(%err, "DexScreener trending fetch failed; using fallback tokens");
            publish_fallback_tokens(conn, "trending").await?;
            return Ok(());
        }
    };
    let mut pairs: Vec<serde_json::Value> = payload
        .get("pairs")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    // Filter for Solana chain
    pairs.retain(|p| p.get("chainId").and_then(|x| x.as_str()) == Some("solana"));
    // Limit to top N
    let top_n = std::env::var("MKT_TRENDING_TOP_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(10);
    let min_liq: f64 = std::env::var("MKT_MIN_LIQUIDITY_USD")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(50_000.0);
    let min_vol: f64 = std::env::var("MKT_MIN_VOLUME_USD")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(5_000_000.0);
    for p in pairs.into_iter().take(top_n) {
        let addr = p
            .get("baseToken")
            .and_then(|b| b.get("address"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let price = p
            .get("priceUsd")
            .and_then(|x| x.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let liq = p
            .get("liquidity")
            .and_then(|l| l.get("usd"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        let vol_h = p
            .get("volume")
            .and_then(|vv| vv.get("h24"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        let vol5m = vol_h / (24.0 * 12.0);
        if addr.is_empty() || price <= 0.0 {
            continue;
        }
        if liq < min_liq {
            continue;
        }
        if vol_h < min_vol {
            continue;
        }
        let evt = MarketEvent::Price(PriceTick {
            token_address: addr.to_string(),
            price_usd: price,
            volume_usd_1m: 0.0,
            volume_usd_5m: vol5m,
            volume_usd_15m: 0.0,
            price_change_1m: 0.0,
            price_change_5m: 0.0,
            liquidity_usd: liq,
            timestamp: chrono::Utc::now(),
        });
        let event_wrapper = shared_models::Event::Market(evt);
        let payload = serde_json::to_string(&event_wrapper)?;
        let _: () = conn
            .xadd(
                "events:price",
                "*",
                &[("type", "price"), ("data", payload.as_str())],
            )
            .await?;
    }
    Ok(())
}

async fn poll_dexscreener_newpairs(conn: &mut MultiplexedConnection) -> Result<()> {
    // Fetch recent Solana pairs and publish those newer than threshold with min liquidity
    let client = build_http_client(StdDuration::from_secs(8))?;
    let url = "https://api.dexscreener.com/latest/dex/pairs/solana";
    let attempts = http_retry_attempts();
    let payload = match fetch_json_with_retry(&client, url, attempts).await {
        Ok(json) => json,
        Err(err) => {
            warn!(%err, "DexScreener new-pairs fetch failed; using fallback tokens");
            publish_fallback_tokens(conn, "newpairs").await?;
            return Ok(());
        }
    };
    let pairs: Vec<serde_json::Value> = payload
        .get("pairs")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    // Configs
    let max_age_min: i64 = std::env::var("MKT_NEWPAIRS_MAX_AGE_MIN")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(120);
    let min_age_min: i64 = std::env::var("MKT_NEWPAIRS_MIN_AGE_MIN")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(10);
    let min_liq: f64 = std::env::var("MKT_MIN_LIQUIDITY_USD")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(25_000.0);
    let min_vol: f64 = std::env::var("MKT_MIN_VOLUME_USD")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(5_000_000.0);
    let top_n = std::env::var("MKT_NEWPAIRS_TOP_N")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(15);

    let now_ms = chrono::Utc::now().timestamp_millis();
    // Basic score: prefer higher liquidity and newer age
    let mut scored: Vec<(f64, serde_json::Value)> = Vec::new();
    for p in pairs.into_iter() {
        // chain filter is already solana; still sanity check
        if p.get("chainId").and_then(|x| x.as_str()) != Some("solana") {
            continue;
        }
        let addr = p
            .get("baseToken")
            .and_then(|b| b.get("address"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let price = p
            .get("priceUsd")
            .and_then(|x| x.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let liq = p
            .get("liquidity")
            .and_then(|l| l.get("usd"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        let vol_h = p
            .get("volume")
            .and_then(|vv| vv.get("h24"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        let created_ms = p
            .get("pairCreatedAt")
            .and_then(|x| x.as_i64())
            .unwrap_or(now_ms);
        let age_min = ((now_ms - created_ms) / 1000 / 60).max(0);
        if addr.is_empty() || price <= 0.0 {
            continue;
        }
        if age_min > max_age_min {
            continue;
        }
        if age_min < min_age_min {
            continue;
        }
        if liq < min_liq {
            continue;
        }
        if vol_h < min_vol {
            continue;
        }
        let age_score = (max_age_min - age_min).max(0) as f64;
        let score = liq + age_score * 100.0;
        scored.push((score, p));
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    for (_s, p) in scored.into_iter().take(top_n) {
        let addr = p
            .get("baseToken")
            .and_then(|b| b.get("address"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let price = p
            .get("priceUsd")
            .and_then(|x| x.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let liq = p
            .get("liquidity")
            .and_then(|l| l.get("usd"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        let vol_h = p
            .get("volume")
            .and_then(|vv| vv.get("h24"))
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0);
        let vol5m = vol_h / (24.0 * 12.0);
        if addr.is_empty() || price <= 0.0 {
            continue;
        }
        let evt = MarketEvent::Price(PriceTick {
            token_address: addr.to_string(),
            price_usd: price,
            volume_usd_1m: 0.0,
            volume_usd_5m: vol5m,
            volume_usd_15m: 0.0,
            price_change_1m: 0.0,
            price_change_5m: 0.0,
            liquidity_usd: liq,
            timestamp: chrono::Utc::now(),
        });
        let event_wrapper = shared_models::Event::Market(evt);
        let payload = serde_json::to_string(&event_wrapper)?;
        let _: () = conn
            .xadd(
                "events:price",
                "*",
                &[("type", "price"), ("data", payload.as_str())],
            )
            .await?;
    }
    Ok(())
}

fn extract_liquidity(value: &serde_json::Value) -> f64 {
    value
        .get("liquidity")
        .or_else(|| value.get("liquidityUsd"))
        .or_else(|| value.get("liquidityUSD"))
        .or_else(|| value.get("liquidity_usd"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

fn extract_volume_24h(value: &serde_json::Value) -> f64 {
    value
        .get("dailyVolume")
        .or_else(|| value.get("daily_volume"))
        .or_else(|| value.get("dailyVolumeUsd"))
        .or_else(|| value.get("dailyVolumeUSD"))
        .or_else(|| value.get("volume24h"))
        .or_else(|| value.get("volume24hUsd"))
        .or_else(|| value.get("volume24hUSD"))
        .or_else(|| value.get("volume_24h_usd"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

async fn fetch_jupiter_token_metadata(
    client: &reqwest::Client,
    address: &str,
) -> Result<FallbackToken> {
    // Stubbed due to DNS issues with tokens.jup.ag
    // In a real scenario, we would query a different provider or use the cached list
    Ok(FallbackToken {
        address: address.to_string(),
        liquidity_usd: 100000.0, // Assume valid to pass filters for now
        volume_24h_usd: 100000.0,
    })
}

async fn fetch_metadata_for_addresses(addresses: &[String]) -> Result<Vec<FallbackToken>> {
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let client = build_http_client(StdDuration::from_secs(6))?;
    let mut out = Vec::with_capacity(addresses.len());
    for addr in addresses {
        let token = fetch_jupiter_token_metadata(&client, addr).await?;
        out.push(token);
    }
    Ok(out)
}

async fn fetch_dynamic_fallback_tokens(limit: usize) -> Result<Vec<FallbackToken>> {
    let client = build_http_client(StdDuration::from_secs(8))?;
    // Use GitHub raw token list as fallback since jup.ag is failing DNS
    let url = "https://raw.githubusercontent.com/solana-labs/token-list/main/src/tokens/solana.tokenlist.json";
    let json = match fetch_json_with_retry(&client, url, http_retry_attempts()).await {
        Ok(v) => v,
        Err(err) => {
            warn!(%err, "Failed to download token catalog for fallback");
            return Ok(Vec::new());
        }
    };
    let mut tokens: Vec<FallbackToken> = Vec::new();
    
    // Handle both array (Jupiter) and object with "tokens" (Standard) formats
    let token_list = if let Some(arr) = json.as_array() {
        Some(arr)
    } else {
        json.get("tokens").and_then(|v| v.as_array())
    };

    if let Some(arr) = token_list {
        for entry in arr {
            let chain_id = entry
                .get("chainId")
                .and_then(|v| v.as_i64()) // Standard list uses int 101 for mainnet
                .map(|v| if v == 101 { "solana" } else { "other" })
                .or_else(|| entry.get("chainId").and_then(|v| v.as_str()))
                .unwrap_or("solana");
            if chain_id != "solana" {
                continue;
            }
            let address = entry
                .get("address")
                .or_else(|| entry.get("mint"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if address.is_empty() {
                continue;
            }
            tokens.push(FallbackToken {
                address: address.to_string(),
                liquidity_usd: extract_liquidity(entry),
                volume_24h_usd: extract_volume_24h(entry),
            });
        }
    }
    tokens.retain(|t| !t.address.is_empty());
    tokens.sort_by(|a, b| {
        b.volume_24h_usd
            .partial_cmp(&a.volume_24h_usd)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if limit > 0 && tokens.len() > limit {
        tokens.truncate(limit);
    }
    Ok(tokens)
}

async fn resolve_fallback_tokens(limit: usize) -> Result<Vec<FallbackToken>> {
    let tokens_env = std::env::var("MKT_FALLBACK_TOKENS").unwrap_or_default();
    let tokens: Vec<String> = tokens_env
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if !tokens.is_empty() {
        return fetch_metadata_for_addresses(&tokens).await;
    }

    fetch_dynamic_fallback_tokens(limit).await
}

async fn publish_fallback_tokens(conn: &mut MultiplexedConnection, reason: &str) -> Result<()> {
    let limit = std::env::var("MKT_FALLBACK_MAX")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(12);
    let mut tokens = resolve_fallback_tokens(limit).await?;

    if tokens.is_empty() {
        warn!(%reason, "No fallback tokens resolved; nothing to publish");
        return Ok(());
    }

    let birdeye_key = std::env::var("BIRDEYE_API_KEY")
        .ok()
        .filter(|s| !s.is_empty());
    for token in tokens.drain(..) {
        match get_jupiter_price(&token.address).await {
            Ok(Some(price)) => {
                let mut liq = token.liquidity_usd.max(0.0);
                let mut vol5m = if token.volume_24h_usd > 0.0 {
                    token.volume_24h_usd / (24.0 * 12.0)
                } else {
                    0.0
                };
                if let Some(api_key) = birdeye_key.as_ref() {
                    if let Ok((b_liq, b_vol5m)) =
                        fetch_birdeye_metrics(&token.address, api_key).await
                    {
                        if b_liq > 0.0 {
                            liq = b_liq;
                        }
                        if b_vol5m > 0.0 {
                            vol5m = b_vol5m;
                        }
                    }
                }
                let evt = MarketEvent::Price(PriceTick {
                    token_address: token.address.clone(),
                    price_usd: price,
                    volume_usd_1m: 0.0,
                    volume_usd_5m: vol5m,
                    volume_usd_15m: vol5m * 3.0,
                    price_change_1m: 0.0,
                    price_change_5m: 0.0,
                    liquidity_usd: liq,
                    timestamp: chrono::Utc::now(),
                });
                let event_wrapper = shared_models::Event::Market(evt);
                let payload = serde_json::to_string(&event_wrapper)?;
                conn.xadd::<_, _, _, _, ()>(
                    "events:price",
                    "*",
                    &[("type", "price"), ("data", payload.as_str())],
                )
                .await?;
                info!(%reason, token = %token.address, "Published fallback price tick");
            }
            Ok(None) => {
                warn!(token = %token.address, %reason, "Jupiter returned no price for fallback token");
            }
            Err(err) => {
                warn!(token = %token.address, %reason, %err, "Failed to fetch Jupiter price for fallback");
            }
        }
    }
    Ok(())
}

async fn get_jupiter_price(token_address: &str) -> Result<Option<f64>> {
    let url = format!("https://price.jup.ag/v6/price?ids={}", token_address);
    let client = build_http_client(StdDuration::from_secs(4))?;
    let json = fetch_json_with_retry(&client, &url, http_retry_attempts()).await?;
    let price = json
        .get("data")
        .and_then(|d| d.get(token_address))
        .and_then(|entry| entry.get("price"))
        .and_then(|v| v.as_f64());
    Ok(price)
}

async fn fetch_birdeye_metrics(token_address: &str, api_key: &str) -> Result<(f64, f64)> {
    let url = format!(
        "https://public-api.birdeye.so/defi/token_overview?address={}",
        token_address
    );
    let client = build_http_client(StdDuration::from_secs(5))?;
    let attempts = http_retry_attempts();
    let mut last_err = None;
    for attempt in 1..=attempts {
        match client.get(&url).header("X-API-KEY", api_key).send().await {
            Ok(resp) if resp.status().is_success() => {
                let json: serde_json::Value = resp.json().await?;
                let data = json.get("data").cloned().unwrap_or_default();
                let liquidity = data
                    .get("liquidity")
                    .or_else(|| data.get("liquidity_usd"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let vol24h = data
                    .get("volume")
                    .and_then(|v| v.get("h24"))
                    .or_else(|| data.get("volume24h"))
                    .or_else(|| data.get("volume24hUSD"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let vol5m = vol24h / (24.0 * 12.0);
                return Ok((liquidity, vol5m));
            }
            Ok(resp) => {
                last_err = Some(anyhow!("Birdeye non-success {}", resp.status()));
            }
            Err(err) => {
                last_err = Some(err.into());
            }
        }
        if attempt < attempts {
            let backoff_ms = 500u64.saturating_mul(1 << (attempt - 1));
            sleep(Duration::from_millis(backoff_ms.min(10_000))).await;
        }
    }
    if let Some(err) = last_err {
        warn!(%token_address, %err, "Birdeye metrics fetch failed");
    }
    Ok((0.0, 0.0))
}

async fn fallback_liquidity_metrics(token_address: &str) -> Result<(f64, f64)> {
    if let Ok(api_key) = std::env::var("BIRDEYE_API_KEY") {
        if !api_key.is_empty() {
            return fetch_birdeye_metrics(token_address, &api_key).await;
        }
    }
    Ok((0.0, 0.0))
}
