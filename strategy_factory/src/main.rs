use anyhow::Result;
use chrono::Utc;
use rand::{seq::SliceRandom, Rng};
use redis::{aio::MultiplexedConnection, AsyncCommands};
use serde_json::json;
use shared_models::StrategySpec;
use std::time::Duration;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, Level};

const REDIS_STREAM: &str = "allocations_channel"; // consumed by executor

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();
    let redis_url =
        std::env::var("REDIS_URL").map_err(|e| anyhow::anyhow!("REDIS_URL not set: {}", e))?;
    let client = redis::Client::open(redis_url)?;
    let mut attempt: u32 = 0;
    let mut conn: MultiplexedConnection = loop {
        match client.get_multiplexed_tokio_connection().await {
            Ok(c) => break c,
            Err(e) => {
                attempt += 1;
                let backoff = TokioDuration::from_secs((attempt.min(10)) as u64);
                tracing::warn!(attempt, %e, "Strategy-Factory: Redis connect failed; retrying in {:?}", backoff);
                sleep(backoff).await;
            }
        }
    };

    info!("🎲 Strategy‑Factory v25 online");

    loop {
        let specs = generate_population(5)?;
        let payload = serde_json::to_string(&specs)?;
        let _: String = conn
            .xadd(REDIS_STREAM, "*", &[("allocations", &payload)])
            .await?;
        info!(
            len = specs.len(),
            "Pushed fresh strategy allocations → Redis"
        );
        sleep(Duration::from_secs(900)).await; // 15 min evolution cadence
    }
}

fn generate_population(n: usize) -> Result<Vec<StrategySpec>> {
    let families = [
        "momentum",
        "meanrevert",
        "social",
        "korean",
        "liquidity",
        "bridge",
        "airdrop",
        "perp",
        "rug",
        "dev",
    ];
    (0..n)
        .map(|_| {
            let fam = *families
                .choose(&mut rand::thread_rng())
                .ok_or_else(|| anyhow::anyhow!("Families array is empty"))?;
            Ok(StrategySpec {
                id: format!("{}_{:x}", fam, rand::thread_rng().gen::<u32>()),
                family: fam.to_string(),
                params: json!({
                    "param1": rand::thread_rng().gen_range(0.1..1.0)
                }),
                fitness: rand::thread_rng().gen_range(0.0..1.0),
                created_at: Utc::now(),
            })
        })
        .collect()
}
