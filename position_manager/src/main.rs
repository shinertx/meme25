use anyhow::Result;
use redis::{aio::MultiplexedConnection, AsyncCommands};
use sqlx::postgres::PgPoolOptions;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct PositionEntry {
    pub symbol: String,
    pub side: String, // "long" or "short"
    pub size: f64,
    pub entry_price: f64,
    pub stop_loss: Option<f64>,
    pub take_profit: Option<f64>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: String,
    pub size: f64,
    pub order_type: String, // "market", "limit"
    pub price: Option<f64>,
    pub strategy_id: String,
}

pub struct PositionManager {
    redis_client: redis::Client,
    positions: HashMap<String, PositionEntry>,
}

impl PositionManager {
    pub fn new(redis_url: &str) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
        Ok(Self { 
            redis_client, 
            positions: HashMap::new() 
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Position Manager");
        let mut con = self.redis_client.get_async_connection().await?;
        
        // Subscribe to trading signals
        tokio::spawn(async move {
            let mut con = match redis::Client::open("redis://redis:6379") {
                Ok(client) => match client.get_async_connection().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        error!("Failed to connect to Redis: {}", e);
                        return;
                    }
                }
                Err(e) => {
                    error!("Failed to create Redis client: {}", e);
                    return;
                }
            };
            
            loop {
                // Listen for trading signals
                match con.xread_options(&["trading_signals"], &[">"], Some(1), Some(1000)).await {
                    Ok(streams) => {
                        for stream in streams {
                            for msg in stream.messages {
                                if let Some(data) = msg.map.get("data") {
                                    if let Ok(signal) = serde_json::from_str::<OrderRequest>(data) {
                                        info!("Received trading signal: {:?}", signal);
                                        // Process the signal
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to read from Redis stream: {}", e);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        // Start position monitoring
        let mut timer = interval(Duration::from_secs(10));
        
        loop {
            timer.tick().await;
            
            if let Err(e) = self.check_stop_losses(&mut con).await {
                error!("Failed to check stop losses: {}", e);
            }
            
            if let Err(e) = self.check_take_profits(&mut con).await {
                error!("Failed to check take profits: {}", e);
            }
        }
    }

    async fn check_stop_losses(&mut self, con: &mut redis::aio::Connection) -> Result<()> {
        let mut positions_to_close = Vec::new();
        
        for (id, position) in &self.positions {
            if let Some(stop_loss) = position.stop_loss {
                // Get current price
                let current_price: Option<String> = con.get(format!("price:{}", position.symbol)).await?;
                
                if let Some(price_str) = current_price {
                    if let Ok(current_price) = price_str.parse::<f64>() {
                        let should_close = match position.side.as_str() {
                            "long" => current_price <= stop_loss,
                            "short" => current_price >= stop_loss,
                            _ => false,
                        };
                        
                        if should_close {
                            warn!("Stop loss triggered for {} at {}", position.symbol, current_price);
                            positions_to_close.push(id.clone());
                        }
                    }
                }
            }
        }
        
        for position_id in positions_to_close {
            self.close_position(&position_id, "stop_loss", con).await?;
        }
        
        Ok(())
    }

    async fn check_take_profits(&mut self, con: &mut redis::aio::Connection) -> Result<()> {
        let mut positions_to_close = Vec::new();
        
        for (id, position) in &self.positions {
            if let Some(take_profit) = position.take_profit {
                // Get current price
                let current_price: Option<String> = con.get(format!("price:{}", position.symbol)).await?;
                
                if let Some(price_str) = current_price {
                    if let Ok(current_price) = price_str.parse::<f64>() {
                        let should_close = match position.side.as_str() {
                            "long" => current_price >= take_profit,
                            "short" => current_price <= take_profit,
                            _ => false,
                        };
                        
                        if should_close {
                            info!("Take profit triggered for {} at {}", position.symbol, current_price);
                            positions_to_close.push(id.clone());
                        }
                    }
                }
            }
        }
        
        for position_id in positions_to_close {
            self.close_position(&position_id, "take_profit", con).await?;
        }
        
        Ok(())
    }

    async fn close_position(&mut self, position_id: &str, reason: &str, con: &mut redis::aio::Connection) -> Result<()> {
        if let Some(position) = self.positions.remove(position_id) {
            let close_signal = serde_json::json!({
                "type": "close_position",
                "symbol": position.symbol,
                "side": if position.side == "long" { "sell" } else { "buy" },
                "size": position.size,
                "reason": reason,
                "timestamp": chrono::Utc::now().timestamp_millis()
            });
            
            let _: () = con.xadd("trading_signals", "*", &[("data", close_signal.to_string())]).await?;
            info!("Closed position {} due to {}", position.symbol, reason);
        }
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new().max_connections(5).connect(&db_url).await?;

    let redis_url = std::env::var("REDIS_URL")?;
    let client = redis::Client::open(redis_url)?;
    let mut conn: MultiplexedConnection = client.get_multiplexed_tokio_connection().await?;

    loop {
        // heartbeat – update unrealised PnL to Prometheus, close stale trades, etc.
        let open: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM trades WHERE status = 'OPEN'")
            .fetch_one(&pool)
            .await?;
        info!(open_positions = open.0, "position‑manager heartbeat");
        sleep(Duration::from_secs(60)).await;

        // TODO: fetch prices & evaluate stops; emit risk events, etc.
        let _: () = conn.hset("pm:stats", "open_positions", open.0).await?;
    }
}
