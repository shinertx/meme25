use anyhow::{Context, Result};
use redis::{
    aio::MultiplexedConnection,
    streams::{StreamReadOptions, StreamReadReply},
    AsyncCommands, Value,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{postgres::PgPoolOptions, FromRow};
use std::collections::HashMap;
use tokio::time::{interval, sleep, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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

fn redis_value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Data(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        Value::Bulk(values) if !values.is_empty() => redis_value_to_string(&values[0]),
        Value::Status(status) => Some(status.clone()),
        Value::Okay => Some("OK".to_string()),
        _ => None,
    }
}

impl PositionManager {
    pub fn new(redis_url: &str) -> Result<Self> {
        let redis_client = redis::Client::open(redis_url)?;
        Ok(Self {
            redis_client,
            positions: HashMap::new(),
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
                },
                Err(e) => {
                    error!("Failed to create Redis client: {}", e);
                    return;
                }
            };

            let read_options = StreamReadOptions::default().count(1).block(1000);

            loop {
                match con
                    .xread_options::<_, _, StreamReadReply>(
                        &["trading_signals"],
                        &[">"],
                        &read_options,
                    )
                    .await
                {
                    Ok(reply) => {
                        for stream in reply.keys {
                            for stream_id in stream.ids {
                                if let Some(value) = stream_id.map.get("data") {
                                    if let Some(payload) = redis_value_to_string(value) {
                                        match serde_json::from_str::<OrderRequest>(&payload) {
                                            Ok(signal) => {
                                                info!("Received trading signal: {:?}", signal);
                                                // Process the signal
                                            }
                                            Err(err) => {
                                                error!("Failed to parse trading signal: {err}");
                                            }
                                        }
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
                let current_price: Option<String> =
                    con.get(format!("price:{}", position.symbol)).await?;

                if let Some(price_str) = current_price {
                    if let Ok(current_price) = price_str.parse::<f64>() {
                        let should_close = match position.side.as_str() {
                            "long" => current_price <= stop_loss,
                            "short" => current_price >= stop_loss,
                            _ => false,
                        };

                        if should_close {
                            warn!(
                                "Stop loss triggered for {} at {}",
                                position.symbol, current_price
                            );
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
                let current_price: Option<String> =
                    con.get(format!("price:{}", position.symbol)).await?;

                if let Some(price_str) = current_price {
                    if let Ok(current_price) = price_str.parse::<f64>() {
                        let should_close = match position.side.as_str() {
                            "long" => current_price >= take_profit,
                            "short" => current_price <= take_profit,
                            _ => false,
                        };

                        if should_close {
                            info!(
                                "Take profit triggered for {} at {}",
                                position.symbol, current_price
                            );
                            positions_to_close.push(id.clone());
                        }
                    }
                }
            }
        }

        for position_id in positions_to_close {
            self.close_position(&position_id, "take_profit", con)
                .await?;
        }

        Ok(())
    }

    async fn close_position(
        &mut self,
        position_id: &str,
        reason: &str,
        con: &mut redis::aio::Connection,
    ) -> Result<()> {
        if let Some(position) = self.positions.remove(position_id) {
            let close_signal = serde_json::json!({
                "type": "close_position",
                "symbol": position.symbol,
                "side": if position.side == "long" { "sell" } else { "buy" },
                "size": position.size,
                "reason": reason,
                "timestamp": chrono::Utc::now().timestamp_millis()
            });

            let _: () = con
                .xadd(
                    "trading_signals",
                    "*",
                    &[("data", close_signal.to_string())],
                )
                .await?;
            info!("Closed position {} due to {}", position.symbol, reason);
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let db_url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

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

        if let Err(e) = evaluate_risk_triggers(&pool, &mut conn, open.0).await {
            error!("failed to evaluate position risk: {e:#}");
        }
    }
}

#[derive(Debug, Clone, FromRow)]
#[allow(dead_code)]
struct OpenTradeRecord {
    trade_uuid: Uuid,
    strategy_id: String,
    token_address: String,
    symbol: String,
    side: String,
    amount_usd: f64,
    entry_price_usd: f64,
    metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Copy)]
enum TriggerReason {
    StopLoss,
    TakeProfit,
}

impl TriggerReason {
    fn as_str(&self) -> &'static str {
        match self {
            TriggerReason::StopLoss => "stop_loss_triggered",
            TriggerReason::TakeProfit => "take_profit_reached",
        }
    }

    fn severity(&self) -> &'static str {
        match self {
            TriggerReason::StopLoss => "HIGH",
            TriggerReason::TakeProfit => "LOW",
        }
    }
}

struct TriggeredClose {
    trade: OpenTradeRecord,
    trigger_price: f64,
    threshold: f64,
    reason: TriggerReason,
}

async fn evaluate_risk_triggers(
    pool: &sqlx::PgPool,
    conn: &mut MultiplexedConnection,
    open_position_count: i64,
) -> Result<()> {
    let records: Vec<OpenTradeRecord> = sqlx::query_as(
        r#"
        SELECT 
            trade_uuid,
            strategy_id,
            token_address,
            symbol,
            side,
            amount_usd,
            entry_price_usd,
            metadata
        FROM trades
        WHERE status = 'OPEN'
        "#,
    )
    .fetch_all(pool)
    .await
    .with_context(|| "failed to load open trades")?;

    if records.is_empty() {
        let _: () = conn
            .hset_multiple(
                "pm:stats",
                &[
                    ("open_positions", open_position_count),
                    ("last_check_ms", chrono::Utc::now().timestamp_millis()),
                ],
            )
            .await?;
        return Ok(());
    }

    let mut triggered: Vec<TriggeredClose> = Vec::new();

    for record in records.into_iter() {
        let stop_loss = extract_price(&record.metadata, &["risk_metrics", "stop_loss_price"])
            .or_else(|| extract_price(&record.metadata, &["stop_loss_price"]))
            .or_else(|| extract_price(&record.metadata, &["stop_loss"]));
        let take_profit = extract_price(&record.metadata, &["risk_metrics", "take_profit_price"])
            .or_else(|| extract_price(&record.metadata, &["take_profit_price"]))
            .or_else(|| extract_price(&record.metadata, &["take_profit"]));

        if stop_loss.is_none() && take_profit.is_none() {
            continue;
        }

        if let Some(current_price) =
            fetch_price(conn, &record.token_address, &record.symbol).await?
        {
            if let Some(stop) = stop_loss {
                if should_trigger(&record.side, current_price, stop, TriggerReason::StopLoss) {
                    triggered.push(TriggeredClose {
                        trade: record,
                        trigger_price: current_price,
                        threshold: stop,
                        reason: TriggerReason::StopLoss,
                    });
                    continue;
                }
            }

            if let Some(take) = take_profit {
                if should_trigger(&record.side, current_price, take, TriggerReason::TakeProfit) {
                    triggered.push(TriggeredClose {
                        trade: record,
                        trigger_price: current_price,
                        threshold: take,
                        reason: TriggerReason::TakeProfit,
                    });
                    continue;
                }
            }
        } else {
            debug!(
                token = %record.token_address,
                symbol = %record.symbol,
                "no price available in redis for open trade"
            );
        }
    }

    for triggered_trade in &triggered {
        publish_close_signal(conn, triggered_trade).await?;
        persist_risk_event(pool, triggered_trade).await?;
    }

    let _: () = conn
        .hset_multiple(
            "pm:stats",
            &[
                ("open_positions", open_position_count),
                ("last_check_ms", chrono::Utc::now().timestamp_millis()),
                ("last_trigger_count", triggered.len() as i64),
            ],
        )
        .await?;

    Ok(())
}

fn extract_price(metadata: &Option<JsonValue>, path: &[&str]) -> Option<f64> {
    let mut current = metadata.as_ref()?;
    if path.is_empty() {
        return None;
    }

    for key in &path[..path.len() - 1] {
        current = current.get(*key)?;
    }

    let value = current.get(path[path.len() - 1])?;

    match value {
        JsonValue::Number(n) => n.as_f64(),
        JsonValue::String(s) => s.parse().ok(),
        _ => None,
    }
}

async fn fetch_price(
    conn: &mut MultiplexedConnection,
    token_address: &str,
    symbol: &str,
) -> Result<Option<f64>> {
    let mut keys = vec![format!("price:{token_address}")];
    if !symbol.is_empty() {
        keys.push(format!("price:{symbol}"));
        keys.push(format!("price:{}", symbol.to_uppercase()));
    }

    for key in keys {
        if let Some(price) = conn.get::<_, Option<String>>(&key).await? {
            if let Ok(value) = price.parse::<f64>() {
                return Ok(Some(value));
            }
        }
    }

    Ok(None)
}

fn should_trigger(side: &str, current_price: f64, threshold: f64, reason: TriggerReason) -> bool {
    match (side, reason) {
        ("Long", TriggerReason::StopLoss) => current_price <= threshold,
        ("Short", TriggerReason::StopLoss) => current_price >= threshold,
        ("Long", TriggerReason::TakeProfit) => current_price >= threshold,
        ("Short", TriggerReason::TakeProfit) => current_price <= threshold,
        _ => false,
    }
}

async fn publish_close_signal(
    conn: &mut MultiplexedConnection,
    triggered: &TriggeredClose,
) -> Result<()> {
    let close_payload = serde_json::json!({
        "type": triggered.reason.as_str(),
        "trade_uuid": triggered.trade.trade_uuid,
        "strategy_id": triggered.trade.strategy_id,
        "token_address": triggered.trade.token_address,
        "symbol": triggered.trade.symbol,
        "side": triggered.trade.side,
        "threshold": triggered.threshold,
        "trigger_price": triggered.trigger_price,
        "timestamp": chrono::Utc::now().timestamp_millis(),
    });

    let payload = close_payload.to_string();
    let _: String = conn
        .xadd(
            "trading_signals",
            "*",
            &[
                ("type", triggered.reason.as_str()),
                ("data", payload.as_str()),
            ],
        )
        .await?;
    info!(
        strategy = %triggered.trade.strategy_id,
        token = %triggered.trade.token_address,
        side = %triggered.trade.side,
        reason = triggered.reason.as_str(),
        threshold = triggered.threshold,
        trigger_price = triggered.trigger_price,
        "emitted close signal for trade"
    );
    Ok(())
}

async fn persist_risk_event(pool: &sqlx::PgPool, triggered: &TriggeredClose) -> Result<()> {
    let description = match triggered.reason {
        TriggerReason::StopLoss => format!(
            "Stop loss triggered for {} at {:.6} (threshold {:.6})",
            triggered.trade.token_address, triggered.trigger_price, triggered.threshold
        ),
        TriggerReason::TakeProfit => format!(
            "Take profit reached for {} at {:.6} (target {:.6})",
            triggered.trade.token_address, triggered.trigger_price, triggered.threshold
        ),
    };

    let metadata = serde_json::json!({
        "trade_uuid": triggered.trade.trade_uuid,
        "token_address": triggered.trade.token_address,
        "symbol": triggered.trade.symbol,
        "strategy_id": triggered.trade.strategy_id,
        "trigger_price": triggered.trigger_price,
        "threshold": triggered.threshold,
        "side": triggered.trade.side,
        "reason": triggered.reason.as_str(),
    });

    sqlx::query(
        r#"
        INSERT INTO risk_events (event_type, severity, strategy_id, description, action_taken, metadata)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(triggered.reason.as_str())
    .bind(triggered.reason.severity())
    .bind(&triggered.trade.strategy_id)
    .bind(&description)
    .bind(Some("close_signal_emitted"))
    .bind(metadata)
    .execute(pool)
    .await
    .with_context(|| "failed to persist risk event")?;

    Ok(())
}
