use anyhow::Result;
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde::{Deserialize, Serialize};
use shared_models::{StrategyAllocation};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{info, Level};

const RESULT_STREAM: &str = "backtest_results";
const OUT_STREAM: &str = "allocations_channel";

#[derive(Debug, Deserialize)]
struct BacktestEnvelope { result: String }

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let redis_url = std::env::var("REDIS_URL")?;
    let client = redis::Client::open(redis_url)?;
    let mut conn: MultiplexedConnection = client.get_multiplexed_tokio_connection().await?;

    info!("🏦 Portfolio‑Manager v25 alive");

    let mut last_id = "$".to_string();
    loop {
        let res: redis::Value = conn
            .xread_options(&[RESULT_STREAM], &[&last_id], &redis::streams::StreamReadOptions::default().block(0))
            .await?;
        if let redis::Value::Bulk(streams) = res {
            for stream in streams {
                if let redis::Value::Bulk(data) = stream {
                    // data[0] = stream key, data[1] = messages
                    if let redis::Value::Bulk(msgs) = &data[1] {
                        for msg in msgs {
                            if let redis::Value::Bulk(parts) = msg {
                                let id = String::from_utf8(parts[0].as_data().unwrap().to_vec()).unwrap();
                                last_id = id.clone();
                                // parts[1] = kv list
                                if let redis::Value::Bulk(kv) = &parts[1] {
                                    if kv.len() >= 2 {
                                        let payload = kv[1].as_data().unwrap();
                                        let env: BacktestEnvelope = serde_json::from_slice(payload)?;
                                        process_result(&env.result, &mut conn).await?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

async fn process_result(raw: &str, conn: &mut MultiplexedConnection) -> Result<()> {
    #[derive(Deserialize)]
    struct R { strategy_id: String, sharpe_ratio: f64, total_return_pct: f64 }
    let r: R = serde_json::from_str(raw)?;

    // Very simple allocation: weight = max(0, sharpe)/sum
    static mut WEIGHTS: HashMap<String, f64> = HashMap::new();
    unsafe { WEIGHTS.insert(r.strategy_id.clone(), (r.sharpe_ratio).max(0.0)); }
    let sum: f64 = unsafe { WEIGHTS.values().sum() };

    let allocs: Vec<StrategyAllocation> = unsafe {
        WEIGHTS
            .iter()
            .map(|(id, w)| StrategyAllocation {
                id: id.clone(),
                weight: if sum > 0.0 { *w / sum } else { 0.0 },
                sharpe_ratio: *w,
                mode: shared_models::TradeMode::Paper,
                params: serde_json::json!({}),
                capital_allocated: 0.0,
                max_position_usd: 20.0,
                current_positions: 0.0,
            })
            .collect()
    };

    let payload = serde_json::to_string(&allocs)?;
    conn.xadd(OUT_STREAM, "*", &["allocations", &payload]).await?;
    info!("Pushed {} allocations", allocs.len());
    Ok(())
}
                capital_allocated: 0.0,
                max_position_usd: 20.0,
                current_positions: 0.0,
            })
            .collect::<Vec<_>>()
    };

    let payload = serde_json::to_string(&allocs)?;
    conn.xadd(OUT_STREAM, "*", &["allocations", &payload]).await?;
    info!("Pushed {} allocations", allocs.len());
    Ok(())
}
        
        loop {
            timer.tick().await;
            
            if let Err(e) = self.update_portfolio(&mut con).await {
                error!("Failed to update portfolio: {}", e);
            }
            
            if let Err(e) = self.publish_portfolio_state(&mut con).await {
                error!("Failed to publish portfolio state: {}", e);
            }
        }
    }

    async fn update_portfolio(&mut self, con: &mut redis::aio::Connection) -> Result<()> {
        // Update position values based on current market prices
        let mut total_position_value = 0.0;
        
        for (symbol, position) in &mut self.portfolio.positions {
            // Get current price from Redis (mock for now)
            let current_price: Option<String> = con.get(format!("price:{}", symbol)).await?;
            
            if let Some(price_str) = current_price {
                if let Ok(current_price) = price_str.parse::<f64>() {
                    let position_value = position.quantity * current_price;
                    position.unrealized_pnl = (current_price - position.avg_price) * position.quantity;
                    total_position_value += position_value;
                }
            }
        }
        
        self.portfolio.total_value = self.portfolio.available_cash + total_position_value;
        self.portfolio.total_pnl = self.portfolio.total_value - 200.0; // Initial capital
        
        Ok(())
    }

    async fn publish_portfolio_state(&self, con: &mut redis::aio::Connection) -> Result<()> {
        let portfolio_json = serde_json::to_string(&self.portfolio)?;
        let _: () = con.set("portfolio:current", &portfolio_json).await?;
        
        // Publish to stream for monitoring
        let event = serde_json::json!({
            "type": "portfolio_update",
            "total_value": self.portfolio.total_value,
            "available_cash": self.portfolio.available_cash,
            "total_pnl": self.portfolio.total_pnl,
            "position_count": self.portfolio.positions.len(),
            "timestamp": chrono::Utc::now().timestamp_millis()
        });
        
        let _: () = con.xadd("portfolio_events", "*", &[("data", event.to_string())]).await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::init();
    
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://localhost:6379".to_string());
    
    let mut portfolio_manager = PortfolioManager::new(&redis_url)?;
    portfolio_manager.start().await?;
    
    Ok(())
}
