use anyhow::Result;
use rand::{seq::SliceRandom, Rng};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde_json::json;
use shared_models::{StrategySpec};
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;
use tracing::{info, Level};

const REDIS_STREAM: &str = "allocations_channel";  // consumed by executor

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not set");
    let client = redis::Client::open(redis_url)?;
    let mut conn: MultiplexedConnection = client.get_multiplexed_tokio_connection().await?;

    info!("🎲 Strategy‑Factory v25 online");

    loop {
        let specs = generate_population(5);
        let payload = serde_json::to_string(&specs)?;
        let _: String = conn.xadd(REDIS_STREAM, "*", &["allocations", &payload]).await?;
        info!(len = specs.len(), "Pushed fresh strategy allocations → Redis");
        sleep(Duration::from_secs(900)).await; // 15 min evolution cadence
    }
}

fn generate_population(n: usize) -> Vec<StrategySpec> {
    let families = [
        "momentum", "meanrevert", "social", "korean", "liquidity", "bridge",
        "airdrop", "perp", "rug", "dev"
    ];
    (0..n)
        .map(|_| {
            let fam = *families.choose(&mut rand::thread_rng()).unwrap();
            StrategySpec {
                id: format!("{}_{:x}", fam, rand::thread_rng().gen::<u32>()),
                family: fam.to_string(),
                params: json!({ "seed": rand::thread_rng().gen::<u64>() }),
                fitness: 0.0,
                created_at: chrono::Utc::now(),
            }
        })
        .collect()
}
