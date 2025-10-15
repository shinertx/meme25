use anyhow::{anyhow, Context, Result};
use redis::{aio::MultiplexedConnection, streams::StreamReadOptions, AsyncCommands, Value};
use serde::Deserialize;
use shared_models::{StrategyAllocation, TradeMode};
use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn, Level};

const RESULT_STREAM: &str = "backtest_results";
const OUT_STREAM: &str = "allocations_channel";
const MAX_POSITION_USD: f64 = 20.0;

#[derive(Debug, Deserialize)]
struct BacktestEnvelope {
    result: String,
}

#[derive(Debug, Deserialize)]
struct BacktestSummary {
    strategy_id: String,
    sharpe_ratio: f64,
    total_return_pct: f64,
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Data(bytes) => std::str::from_utf8(bytes).ok().map(str::to_string),
        Value::Status(s) => Some(s.clone()),
        Value::Okay => Some("OK".to_string()),
        Value::Bulk(values) if !values.is_empty() => value_to_string(&values[0]),
        _ => None,
    }
}

struct PortfolioAllocator {
    weights: HashMap<String, f64>,
    mode: TradeMode,
    max_position_usd: f64,
}

impl PortfolioAllocator {
    fn new(mode: TradeMode, max_position_usd: f64) -> Self {
        Self {
            weights: HashMap::new(),
            mode,
            max_position_usd,
        }
    }

    fn incorporate(&mut self, summary: &BacktestSummary) -> Vec<StrategyAllocation> {
        let weight = summary.sharpe_ratio.max(0.0);
        self.weights.insert(summary.strategy_id.clone(), weight);

        let total_weight: f64 = self.weights.values().copied().sum();
        let normalizer = if total_weight > f64::EPSILON {
            total_weight
        } else {
            1.0
        };

        self.weights
            .iter()
            .map(|(id, w)| StrategyAllocation {
                id: id.clone(),
                weight: if *w > 0.0 { *w / normalizer } else { 0.0 },
                sharpe_ratio: *w,
                mode: self.mode,
                params: serde_json::json!({"source": "backtest_v25"}),
                capital_allocated: 0.0,
                max_position_usd: self.max_position_usd,
                current_positions: 0.0,
            })
            .collect()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    let redis_url = std::env::var("REDIS_URL")
        .with_context(|| "REDIS_URL env var missing for portfolio-manager")?;
    let client = redis::Client::open(redis_url.clone())
        .with_context(|| format!("Failed to create Redis client for {redis_url}"))?;

    // Robust Redis connection with retries (handles transient DNS failures)
    let mut attempt: u32 = 0;
    let mut conn: MultiplexedConnection = loop {
        match client.get_multiplexed_tokio_connection().await {
            Ok(c) => break c,
            Err(e) => {
                attempt += 1;
                let backoff = Duration::from_secs((attempt.min(10)) as u64);
                warn!(attempt, %e, "Redis connect failed; retrying in {:?}", backoff);
                sleep(backoff).await;
            }
        }
    };

    info!(redis_url = %redis_url, "🏦 Portfolio-Manager v25 alive");

    let mut allocator = PortfolioAllocator::new(TradeMode::Paper, MAX_POSITION_USD);
    let mut last_id = "0-0".to_string();

    loop {
        let response = match conn
            .xread_options(
                &[RESULT_STREAM],
                &[&last_id],
                &StreamReadOptions::default().block(1000).count(64),
            )
            .await
        {
            Ok(value) => value,
            Err(err) => {
                if err.kind() == redis::ErrorKind::ResponseError
                    && err.to_string().contains("Invalid stream ID")
                {
                    warn!(error = %err, last_id, "Redis stream not initialized; resetting cursor");
                    last_id = "0-0".to_string();
                } else {
                    warn!(error = %err, last_id, "Redis stream read failed; retrying");
                }
                sleep(Duration::from_millis(500)).await;
                continue;
            }
        };

        match response {
            Value::Bulk(streams) => {
                for stream in streams {
                    let Value::Bulk(stream_parts) = stream else {
                        continue;
                    };
                    if stream_parts.len() < 2 {
                        continue;
                    }

                    let Some(Value::Bulk(entries)) = stream_parts.get(1) else {
                        continue;
                    };

                    for entry in entries {
                        let Value::Bulk(entry_parts) = entry else {
                            continue;
                        };
                        if entry_parts.len() < 2 {
                            continue;
                        }
                        let Some(entry_id) = value_to_string(&entry_parts[0]) else {
                            error!("Skipping entry without id");
                            continue;
                        };
                        debug!(entry_id = %entry_id, "Processing backtest entry");
                        last_id = entry_id;

                        if let Value::Bulk(key_values) = &entry_parts[1] {
                            if let Err(err) =
                                handle_entry(&key_values[..], &mut allocator, &mut conn).await
                            {
                                error!(error = %err, "Failed processing backtest entry");
                            }
                        }
                    }
                }
            }
            Value::Nil => {
                // No new data within the block timeout; continue polling.
                continue;
            }
            other => {
                warn!("Unexpected Redis response: {other:?}");
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

async fn handle_entry(
    key_values: &[Value],
    allocator: &mut PortfolioAllocator,
    conn: &mut MultiplexedConnection,
) -> Result<()> {
    let mut idx = 0;
    while idx + 1 < key_values.len() {
        let field = &key_values[idx];
        let value = &key_values[idx + 1];
        idx += 2;

        let Some(field_name) = value_to_string(field) else {
            continue;
        };

        if field_name != "result" {
            continue;
        }

        let payload =
            value_to_string(value).ok_or_else(|| anyhow!("Result field missing payload"))?;

        let envelope: BacktestEnvelope = match serde_json::from_str(&payload) {
            Ok(envelope) => envelope,
            Err(err) => {
                warn!(%err, "Skipping malformed backtest envelope");
                continue;
            }
        };
        let summary: BacktestSummary = match serde_json::from_str(&envelope.result) {
            Ok(summary) => summary,
            Err(err) => {
                warn!(%err, "Skipping malformed backtest summary");
                continue;
            }
        };

        let allocations = allocator.incorporate(&summary);
        publish_allocations(conn, &allocations).await?;
        info!(
            strategy_id = %summary.strategy_id,
            sharpe = summary.sharpe_ratio,
            total_return_pct = summary.total_return_pct,
            alloc_count = allocations.len(),
            "updated allocations"
        );
    }

    Ok(())
}

async fn publish_allocations(
    conn: &mut MultiplexedConnection,
    allocations: &[StrategyAllocation],
) -> Result<()> {
    if allocations.is_empty() {
        return Ok(());
    }

    let payload = serde_json::to_string(allocations).context("Serializing allocations")?;
    conn.xadd::<_, _, _, _, ()>(OUT_STREAM, "*", &[("allocations", payload)])
        .await
        .context("Failed to publish allocations")?;
    Ok(())
}
