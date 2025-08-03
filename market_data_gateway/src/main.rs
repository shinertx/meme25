use anyhow::Result;
use async_tungstenite::{tokio::connect_async, tungstenite::Message};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::Deserialize;
use shared_models::{MarketEvent, PriceTick, EventType};
use tokio::time::{sleep, Duration, interval};
use tracing::{info, warn, error};
use futures_util::{StreamExt, SinkExt};

#[derive(Debug, Deserialize)]
struct HeliusSlotTick {
    #[serde(rename = "price"  )]
    price_usd: f64,
    symbol:     String,
    address:    String,
    #[serde(with = "chrono::serde::ts_seconds")]
    timestamp:  chrono::DateTime<chrono::Utc>,
}

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
                if let Err(e) = collect_pump_fun_data(&mut con).await {
                    error!("Failed to collect Pump.fun data: {}", e);
                }
                
                if let Err(e) = collect_solana_data(&mut con).await {
                    error!("Failed to collect Solana data: {}", e);
                }
            }
        });

        Ok(())
    }
}

async fn collect_pump_fun_data(con: &mut redis::aio::Connection) -> Result<()> {
    // Mock data collection - replace with actual API calls
    let mock_data = serde_json::json!({
        "type": "price",
        "symbol": "MEME",
        "price": 0.001234,
        "volume": 50000.0,
        "timestamp": chrono::Utc::now().timestamp_millis()
    });
    
    let _: () = con.xadd("market_events", "*", &[("data", mock_data.to_string())]).await?;
    Ok(())
}

async fn collect_solana_data(con: &mut redis::aio::Connection) -> Result<()> {
    // Mock data collection - replace with actual API calls
    let mock_data = serde_json::json!({
        "type": "transaction",
        "from": "11111111111111111111111111111112",
        "to": "11111111111111111111111111111113",
        "amount": 1000000,
        "timestamp": chrono::Utc::now().timestamp_millis()
    });
    
    let _: () = con.xadd("market_events", "*", &[("data", mock_data.to_string())]).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let redis_url = std::env::var("REDIS_URL")?;
    let client = redis::Client::open(redis_url)?;
    let mut conn: MultiplexedConnection = client.get_multiplexed_tokio_connection().await?;

    let helius_ws = std::env::var("HELIUS_API_KEY")?;
    let url = format!("wss://stream.helius-rpc.com/v0?api-key={}&subscriptions=trades", helius_ws);
    loop {
        match connect_async(&url).await {
            Ok((ws, _)) => {
                info!("Helius WS connected");
                let (mut write, mut read) = ws.split();
                // send nothing; default subscription already set in url
                while let Some(msg) = read.next().await {
                    if let Ok(Message::Text(txt)) = msg {
                        if let Ok(tick) = serde_json::from_str::<HeliusSlotTick>(&txt) {
                            forward_price(&mut conn, tick).await?;
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
}

async fn forward_price(conn: &mut MultiplexedConnection, tick: HeliusSlotTick) -> Result<()> {
    let evt = MarketEvent::Price(PriceTick {
        token_address: tick.address.clone(),
        price_usd: tick.price_usd,
        volume_usd_1m: 0.0,
        volume_usd_5m: 0.0,
        volume_usd_15m: 0.0,
        price_change_1m: 0.0,
        price_change_5m: 0.0,
        liquidity_usd: 0.0,
        timestamp: tick.timestamp,
    });
    let payload = serde_json::to_string(&evt)?;
    conn.xadd("events:price", "*", &["data", &payload]).await?;
    Ok(())
}
