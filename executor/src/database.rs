use anyhow::{Result, Context};
use sqlx::{PgPool, Row};
use shared_models::{Trade, StrategyPerformance, RiskEvent, CapitalAllocation};
use std::collections::HashMap;
use tokio::sync::RwLock;

pub enum Database {
    Live { pool: PgPool },
    Mock { 
        trades: RwLock<Vec<Trade>>,
        performance: RwLock<HashMap<String, StrategyPerformance>>,
        allocations: RwLock<Vec<CapitalAllocation>>,
    },
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        // Check if we're in paper trading mode
        if std::env::var("PAPER_TRADING_MODE").unwrap_or_default() == "true" {
            tracing::info!("📝 Using MockDatabase for paper trading mode");
            return Ok(Self::Mock {
                trades: RwLock::new(Vec::new()),
                performance: RwLock::new(HashMap::new()),
                allocations: RwLock::new(Vec::new()),
            });
        }
        
        let pool = PgPool::connect(database_url)
            .await
            .with_context(|| format!("Failed to connect to database: {}", database_url))?;
        
        tracing::info!("🗄️ Connected to live PostgreSQL database");
        Ok(Self::Live { pool })
    }

    pub async fn save_trade(&self, trade: &Trade) -> Result<()> {
        match self {
            Database::Live { pool } => {
                sqlx::query(
                    r#"
                    INSERT INTO trades (id, strategy_id, symbol, side, quantity, price, timestamp, profit_loss)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                    "#
                )
                .bind(&trade.id)
                .bind(&trade.strategy_id)
                .bind(&trade.symbol)
                .bind(trade.side.to_string())
                .bind(trade.quantity)
                .bind(trade.price)
                .bind(trade.timestamp)
                .bind(trade.profit_loss)
                .execute(pool)
                .await
                .with_context(|| "Failed to save trade to database")?;
            }
            Database::Mock { trades, .. } => {
                let mut trades_guard = trades.write().await;
                trades_guard.push(trade.clone());
                tracing::info!("📝 Mock saved trade: {} {} {}", trade.side, trade.quantity, trade.symbol);
            }
        }
        Ok(())
    }

    pub async fn save_strategy_performance(&self, performance: &StrategyPerformance) -> Result<()> {
        match self {
            Database::Live { pool } => {
                sqlx::query(
                    r#"
                    INSERT INTO strategy_performance (strategy_id, total_trades, winning_trades, total_pnl_usd, sharpe_ratio, sortino_ratio, max_drawdown_pct, win_rate, profit_factor, avg_win_usd, avg_loss_usd, last_updated)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ON CONFLICT (strategy_id) DO UPDATE SET
                        total_trades = EXCLUDED.total_trades,
                        winning_trades = EXCLUDED.winning_trades,
                        total_pnl_usd = EXCLUDED.total_pnl_usd,
                        sharpe_ratio = EXCLUDED.sharpe_ratio,
                        sortino_ratio = EXCLUDED.sortino_ratio,
                        max_drawdown_pct = EXCLUDED.max_drawdown_pct,
                        win_rate = EXCLUDED.win_rate,
                        profit_factor = EXCLUDED.profit_factor,
                        avg_win_usd = EXCLUDED.avg_win_usd,
                        avg_loss_usd = EXCLUDED.avg_loss_usd,
                        last_updated = EXCLUDED.last_updated
                    "#
                )
                .bind(&performance.strategy_id)
                .bind(performance.total_trades as i32)
                .bind(performance.winning_trades as i32)
                .bind(performance.total_pnl_usd)
                .bind(performance.sharpe_ratio)
                .bind(performance.sortino_ratio)
                .bind(performance.max_drawdown_pct)
                .bind(performance.win_rate)
                .bind(performance.profit_factor)
                .bind(performance.avg_win_usd)
                .bind(performance.avg_loss_usd)
                .bind(performance.last_updated)
                .execute(pool)
                .await
                .with_context(|| "Failed to save strategy performance")?;
            }
            Database::Mock { performance: perf_map, .. } => {
                let mut perf_guard = perf_map.write().await;
                perf_guard.insert(performance.strategy_id.clone(), performance.clone());
                tracing::info!("📝 Mock saved performance for strategy: {}", performance.strategy_id);
            }
        }
        Ok(())
    }

    pub async fn save_risk_event(&self, event: &RiskEvent) -> Result<()> {
        match self {
            Database::Live { pool } => {
                sqlx::query(
                    r#"
                    INSERT INTO risk_events (id, event_type, severity, description, timestamp, strategy_id)
                    VALUES ($1, $2, $3, $4, $5, $6)
                    "#
                )
                .bind(&event.id)
                .bind(event.event_type.to_string())
                .bind(event.severity.to_string())
                .bind(&event.description)
                .bind(event.timestamp)
                .bind(&event.strategy_id)
                .execute(pool)
                .await
                .with_context(|| "Failed to save risk event")?;
            }
            Database::Mock { .. } => {
                tracing::info!("📝 Mock saved risk event: {}", event.description);
            }
        }
        Ok(())
    }

    pub async fn save_capital_allocation(&self, allocation: &CapitalAllocation) -> Result<()> {
        match self {
            Database::Live { pool } => {
                sqlx::query(
                    r#"
                    INSERT INTO capital_allocations (id, strategy_id, allocated_capital, timestamp, notes)
                    VALUES ($1, $2, $3, $4, $5)
                    "#
                )
                .bind(&allocation.id)
                .bind(&allocation.strategy_id)
                .bind(allocation.allocated_capital)
                .bind(allocation.timestamp)
                .bind(&allocation.notes)
                .execute(pool)
                .await
                .with_context(|| "Failed to save capital allocation")?;
            }
            Database::Mock { allocations, .. } => {
                let mut alloc_guard = allocations.write().await;
                alloc_guard.push(allocation.clone());
                tracing::info!("📝 Mock saved allocation: {} for {}", allocation.allocated_capital, allocation.strategy_id);
            }
        }
        Ok(())
    }

    pub async fn get_recent_trades(&self, strategy_id: &str, limit: i64) -> Result<Vec<Trade>> {
        match self {
            Database::Live { pool } => {
                let rows = sqlx::query(
                    r#"
                    SELECT id, strategy_id, symbol, side, quantity, price, timestamp, profit_loss
                    FROM trades 
                    WHERE strategy_id = $1 
                    ORDER BY timestamp DESC 
                    LIMIT $2
                    "#
                )
                .bind(strategy_id)
                .bind(limit)
                .fetch_all(pool)
                .await
                .with_context(|| "Failed to fetch recent trades")?;
                
                // Convert rows to Trade objects
                let mut trades = Vec::new();
                for row in rows {
                    let side_str: String = row.get("side");
                    let side = match side_str.as_str() {
                        "Long" => shared_models::Side::Long,
                        "Short" => shared_models::Side::Short,
                        _ => shared_models::Side::Long, // Default fallback
                    };
                    
                    let trade = Trade {
                        id: row.get("id"),
                        strategy_id: row.get("strategy_id"),
                        symbol: row.get("symbol"),
                        side,
                        quantity: row.get("quantity"),
                        price: row.get("price"),
                        timestamp: row.get("timestamp"),
                        profit_loss: row.get("profit_loss"),
                    };
                    trades.push(trade);
                }
                Ok(trades)
            }
            Database::Mock { trades, .. } => {
                let trades_guard = trades.read().await;
                let filtered_trades: Vec<Trade> = trades_guard
                    .iter()
                    .filter(|t| t.strategy_id == strategy_id)
                    .take(limit as usize)
                    .cloned()
                    .collect();
                Ok(filtered_trades)
            }
        }
    }
}
