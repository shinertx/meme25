use chrono::{DateTime, Duration, Utc};
use prometheus::{Counter, Gauge, Histogram, HistogramOpts, Opts, Registry};
use serde::{Deserialize, Serialize};
use shared_models::error::Result;
use shared_models::Side;
use std::collections::{BTreeMap, HashMap, VecDeque};
use tracing::{debug, info};

/// Real-time P&L tracking with institutional-grade attribution and reporting
#[derive(Debug)]
pub struct PnLTracker {
    // Position tracking
    positions: HashMap<String, Position>,

    // Performance tracking
    daily_pnl: f64,
    cumulative_pnl: f64,
    unrealized_pnl: f64,
    high_water_mark: f64,
    max_drawdown: f64,

    // Strategy attribution
    strategy_pnl: HashMap<String, StrategyPnL>,

    // Time series data
    pnl_history: VecDeque<PnLSnapshot>,
    intraday_pnl: BTreeMap<DateTime<Utc>, f64>,

    // Risk metrics
    var_95: f64,
    sharpe_ratio: f64,
    win_rate: f64,
    _average_win: f64,
    _average_loss: f64,

    // Prometheus metrics
    total_pnl_gauge: Gauge,
    daily_pnl_gauge: Gauge,
    unrealized_pnl_gauge: Gauge,
    drawdown_gauge: Gauge,
    sharpe_gauge: Gauge,
    win_rate_gauge: Gauge,
    position_count_gauge: Gauge,
    total_trades_counter: Counter,
    winning_trades_counter: Counter,
    pnl_histogram: Histogram,

    // Configuration
    portfolio_start_value: f64,
    risk_free_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: Side,
    pub quantity: f64,
    pub average_price: f64,
    pub current_price: f64,
    pub market_value: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub entry_timestamp: DateTime<Utc>,
    pub strategy_id: String,
    pub commission_paid: f64,
    pub cost_basis: f64,
    pub duration_hours: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPnL {
    pub strategy_id: String,
    pub realized_pnl: f64,
    pub unrealized_pnl: f64,
    pub total_pnl: f64,
    pub trade_count: u32,
    pub win_count: u32,
    pub loss_count: u32,
    pub average_trade_pnl: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
    pub current_drawdown: f64,
    pub capital_allocated: f64,
    pub return_on_capital: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_pnl: f64,
    pub daily_pnl: f64,
    pub unrealized_pnl: f64,
    pub portfolio_value: f64,
    pub position_count: u32,
    pub cash_balance: f64,
    pub drawdown: f64,
    pub var_95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLReport {
    pub timestamp: DateTime<Utc>,
    pub summary: PnLSummary,
    pub positions: Vec<Position>,
    pub strategy_attribution: Vec<StrategyPnL>,
    pub daily_performance: Vec<PnLSnapshot>,
    pub risk_metrics: RiskMetrics,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLSummary {
    pub total_pnl: f64,
    pub daily_pnl: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub portfolio_value: f64,
    pub cash_balance: f64,
    pub total_return_pct: f64,
    pub daily_return_pct: f64,
    pub position_count: u32,
    pub active_strategies: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMetrics {
    pub value_at_risk_95: f64,
    pub max_drawdown: f64,
    pub current_drawdown: f64,
    pub sharpe_ratio: f64,
    pub sortino_ratio: f64,
    pub calmar_ratio: f64,
    pub volatility: f64,
    pub beta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: f64,
    pub average_win: f64,
    pub average_loss: f64,
    pub profit_factor: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub average_trade_duration_hours: f64,
    pub trades_per_day: f64,
}

impl PnLTracker {
    pub fn new(portfolio_start_value: f64, registry: &Registry) -> Result<Self> {
        // Initialize Prometheus metrics
        let total_pnl_gauge = Gauge::with_opts(Opts::new(
            "portfolio_total_pnl_usd",
            "Total portfolio P&L in USD",
        ))?;

        let daily_pnl_gauge = Gauge::with_opts(Opts::new(
            "portfolio_daily_pnl_usd",
            "Daily portfolio P&L in USD",
        ))?;

        let unrealized_pnl_gauge = Gauge::with_opts(Opts::new(
            "portfolio_unrealized_pnl_usd",
            "Unrealized portfolio P&L in USD",
        ))?;

        let drawdown_gauge = Gauge::with_opts(Opts::new(
            "portfolio_drawdown_pct",
            "Current portfolio drawdown percentage",
        ))?;

        let sharpe_gauge = Gauge::with_opts(Opts::new(
            "portfolio_sharpe_ratio",
            "Portfolio Sharpe ratio",
        ))?;

        let win_rate_gauge = Gauge::with_opts(Opts::new(
            "portfolio_win_rate_pct",
            "Portfolio win rate percentage",
        ))?;

        let position_count_gauge = Gauge::with_opts(Opts::new(
            "portfolio_position_count",
            "Number of active positions",
        ))?;

        let total_trades_counter = Counter::with_opts(Opts::new(
            "portfolio_total_trades",
            "Total number of trades executed",
        ))?;

        let winning_trades_counter = Counter::with_opts(Opts::new(
            "portfolio_winning_trades",
            "Total number of winning trades",
        ))?;

        let pnl_histogram = Histogram::with_opts(
            HistogramOpts::new(
                "portfolio_trade_pnl_usd",
                "Distribution of trade P&L in USD",
            )
            .buckets(vec![
                -100.0, -50.0, -20.0, -10.0, -5.0, 0.0, 5.0, 10.0, 20.0, 50.0, 100.0,
            ]),
        )?;

        // Register all metrics
        registry.register(Box::new(total_pnl_gauge.clone()))?;
        registry.register(Box::new(daily_pnl_gauge.clone()))?;
        registry.register(Box::new(unrealized_pnl_gauge.clone()))?;
        registry.register(Box::new(drawdown_gauge.clone()))?;
        registry.register(Box::new(sharpe_gauge.clone()))?;
        registry.register(Box::new(win_rate_gauge.clone()))?;
        registry.register(Box::new(position_count_gauge.clone()))?;
        registry.register(Box::new(total_trades_counter.clone()))?;
        registry.register(Box::new(winning_trades_counter.clone()))?;
        registry.register(Box::new(pnl_histogram.clone()))?;

        Ok(Self {
            positions: HashMap::new(),
            daily_pnl: 0.0,
            cumulative_pnl: 0.0,
            unrealized_pnl: 0.0,
            high_water_mark: portfolio_start_value,
            max_drawdown: 0.0,
            strategy_pnl: HashMap::new(),
            pnl_history: VecDeque::with_capacity(1000),
            intraday_pnl: BTreeMap::new(),
            var_95: 0.0,
            sharpe_ratio: 0.0,
            win_rate: 0.0,
            _average_win: 0.0,
            _average_loss: 0.0,
            total_pnl_gauge,
            daily_pnl_gauge,
            unrealized_pnl_gauge,
            drawdown_gauge,
            sharpe_gauge,
            win_rate_gauge,
            position_count_gauge,
            total_trades_counter,
            winning_trades_counter,
            pnl_histogram,
            portfolio_start_value,
            risk_free_rate: 0.04, // 4% risk-free rate
        })
    }

    /// Record a new trade execution
    pub fn record_trade(
        &mut self,
        symbol: &str,
        side: Side,
        quantity: f64,
        price: f64,
        commission: f64,
        strategy_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        // Update position
        let position = self
            .positions
            .entry(symbol.to_string())
            .or_insert(Position {
                symbol: symbol.to_string(),
                side: side.clone(),
                quantity: 0.0,
                average_price: 0.0,
                current_price: price,
                market_value: 0.0,
                unrealized_pnl: 0.0,
                realized_pnl: 0.0,
                entry_timestamp: timestamp,
                strategy_id: strategy_id.to_string(),
                commission_paid: 0.0,
                cost_basis: 0.0,
                duration_hours: 0.0,
            });

        // Handle position updates based on side
        let _trade_value = quantity * price;
        let signed_quantity = match side {
            Side::Long => quantity,
            Side::Short => -quantity,
        };

        // Calculate realized P&L for closing trades
        let mut realized_pnl = 0.0;

        if (position.quantity > 0.0 && signed_quantity < 0.0)
            || (position.quantity < 0.0 && signed_quantity > 0.0)
        {
            // Closing or reducing position
            let closing_quantity = signed_quantity.abs().min(position.quantity.abs());
            realized_pnl = match position.side {
                Side::Long => (price - position.average_price) * closing_quantity,
                Side::Short => (position.average_price - price) * closing_quantity,
            };

            position.realized_pnl += realized_pnl;
            self.cumulative_pnl += realized_pnl;
            self.daily_pnl += realized_pnl;
        }

        // Update position quantity and average price
        if position.quantity == 0.0 {
            // New position
            position.quantity = signed_quantity;
            position.average_price = price;
            position.side = side;
            position.entry_timestamp = timestamp;
            position.strategy_id = strategy_id.to_string();
        } else if (position.quantity > 0.0 && signed_quantity > 0.0)
            || (position.quantity < 0.0 && signed_quantity < 0.0)
        {
            // Adding to position
            let total_cost = position.quantity * position.average_price + signed_quantity * price;
            position.quantity += signed_quantity;
            if position.quantity != 0.0 {
                position.average_price = total_cost / position.quantity;
            }
        } else {
            // Reducing position
            position.quantity += signed_quantity;
            if position.quantity.abs() < 0.0001 {
                position.quantity = 0.0;
            }
        }

        // Update position metrics
        position.commission_paid += commission;
        position.cost_basis = position.quantity.abs() * position.average_price;
        position.market_value = position.quantity * price;
        position.current_price = price;
        position.duration_hours = timestamp
            .signed_duration_since(position.entry_timestamp)
            .num_milliseconds() as f64
            / 3_600_000.0;

        // Update strategy P&L tracking
        let strategy_pnl =
            self.strategy_pnl
                .entry(strategy_id.to_string())
                .or_insert(StrategyPnL {
                    strategy_id: strategy_id.to_string(),
                    realized_pnl: 0.0,
                    unrealized_pnl: 0.0,
                    total_pnl: 0.0,
                    trade_count: 0,
                    win_count: 0,
                    loss_count: 0,
                    average_trade_pnl: 0.0,
                    best_trade: 0.0,
                    worst_trade: 0.0,
                    sharpe_ratio: 0.0,
                    max_drawdown: 0.0,
                    current_drawdown: 0.0,
                    capital_allocated: 0.0,
                    return_on_capital: 0.0,
                });

        strategy_pnl.trade_count += 1;
        strategy_pnl.realized_pnl += realized_pnl;

        if realized_pnl > 0.0 {
            strategy_pnl.win_count += 1;
            self.winning_trades_counter.inc();
            if realized_pnl > strategy_pnl.best_trade {
                strategy_pnl.best_trade = realized_pnl;
            }
        } else if realized_pnl < 0.0 {
            strategy_pnl.loss_count += 1;
            if realized_pnl < strategy_pnl.worst_trade {
                strategy_pnl.worst_trade = realized_pnl;
            }
        }

        // Update metrics
        self.total_trades_counter.inc();
        self.pnl_histogram.observe(realized_pnl);

        // Clean up empty positions
        if position.quantity.abs() < 0.0001 {
            self.positions.remove(symbol);
        }

        debug!(
            symbol = symbol,
            side = ?side,
            quantity = quantity,
            price = price,
            realized_pnl = realized_pnl,
            strategy_id = strategy_id,
            "Trade recorded in P&L tracker"
        );

        Ok(())
    }

    /// Update current market prices for all positions
    pub fn update_prices(&mut self, price_updates: HashMap<String, f64>) -> Result<()> {
        let mut total_unrealized = 0.0;

        for (symbol, new_price) in price_updates {
            if let Some(position) = self.positions.get_mut(&symbol) {
                position.current_price = new_price;
                position.market_value = position.quantity * new_price;

                // Calculate unrealized P&L
                position.unrealized_pnl = match position.side {
                    Side::Long => (new_price - position.average_price) * position.quantity,
                    Side::Short => (position.average_price - new_price) * position.quantity.abs(),
                };

                total_unrealized += position.unrealized_pnl;

                // Update strategy unrealized P&L
                if let Some(strategy_pnl) = self.strategy_pnl.get_mut(&position.strategy_id) {
                    strategy_pnl.unrealized_pnl += position.unrealized_pnl;
                }
            }
        }

        self.unrealized_pnl = total_unrealized;

        // Update portfolio metrics
        self.update_portfolio_metrics();

        Ok(())
    }

    /// Take a snapshot of current P&L state
    pub fn take_snapshot(&mut self, timestamp: DateTime<Utc>) -> Result<()> {
        let portfolio_value =
            self.portfolio_start_value + self.cumulative_pnl + self.unrealized_pnl;
        let current_drawdown = self.calculate_current_drawdown();

        let snapshot = PnLSnapshot {
            timestamp,
            total_pnl: self.cumulative_pnl + self.unrealized_pnl,
            daily_pnl: self.daily_pnl,
            unrealized_pnl: self.unrealized_pnl,
            portfolio_value,
            position_count: self.positions.len() as u32,
            cash_balance: self.portfolio_start_value + self.cumulative_pnl
                - self.calculate_invested_capital(),
            drawdown: current_drawdown,
            var_95: self.var_95,
        };

        self.pnl_history.push_back(snapshot);

        // Keep only recent history (last 30 days)
        let cutoff = timestamp - Duration::days(30);
        while let Some(front) = self.pnl_history.front() {
            if front.timestamp < cutoff {
                self.pnl_history.pop_front();
            } else {
                break;
            }
        }

        // Update intraday P&L tracking
        self.intraday_pnl.insert(timestamp, self.daily_pnl);

        info!(
            total_pnl = self.cumulative_pnl + self.unrealized_pnl,
            daily_pnl = self.daily_pnl,
            portfolio_value = portfolio_value,
            position_count = self.positions.len(),
            "P&L snapshot taken"
        );

        Ok(())
    }

    /// Reset daily P&L tracking (call at start of new trading day)
    pub fn reset_daily_pnl(&mut self) {
        self.daily_pnl = 0.0;
        self.intraday_pnl.clear();

        // Reset strategy daily tracking
        for strategy_pnl in self.strategy_pnl.values_mut() {
            // Update running averages and metrics but reset daily tracking
            strategy_pnl.total_pnl = strategy_pnl.realized_pnl + strategy_pnl.unrealized_pnl;
            if strategy_pnl.trade_count > 0 {
                strategy_pnl.average_trade_pnl =
                    strategy_pnl.realized_pnl / strategy_pnl.trade_count as f64;
            }
        }

        info!("Daily P&L tracking reset");
    }

    /// Generate comprehensive P&L report
    pub fn generate_report(&self) -> PnLReport {
        let total_pnl = self.cumulative_pnl + self.unrealized_pnl;
        let portfolio_value = self.portfolio_start_value + total_pnl;
        let cash_balance =
            self.portfolio_start_value + self.cumulative_pnl - self.calculate_invested_capital();

        let summary = PnLSummary {
            total_pnl,
            daily_pnl: self.daily_pnl,
            unrealized_pnl: self.unrealized_pnl,
            realized_pnl: self.cumulative_pnl,
            portfolio_value,
            cash_balance,
            total_return_pct: (total_pnl / self.portfolio_start_value) * 100.0,
            daily_return_pct: (self.daily_pnl / portfolio_value) * 100.0,
            position_count: self.positions.len() as u32,
            active_strategies: self.strategy_pnl.len() as u32,
        };

        let risk_metrics = self.calculate_risk_metrics();
        let performance_metrics = self.calculate_performance_metrics();

        PnLReport {
            timestamp: Utc::now(),
            summary,
            positions: self.positions.values().cloned().collect(),
            strategy_attribution: self.strategy_pnl.values().cloned().collect(),
            daily_performance: self.pnl_history.iter().cloned().collect(),
            risk_metrics,
            performance_metrics,
        }
    }

    /// Update all Prometheus metrics
    pub fn update_metrics(&self) {
        let total_pnl = self.cumulative_pnl + self.unrealized_pnl;
        let current_drawdown = self.calculate_current_drawdown();

        self.total_pnl_gauge.set(total_pnl);
        self.daily_pnl_gauge.set(self.daily_pnl);
        self.unrealized_pnl_gauge.set(self.unrealized_pnl);
        self.drawdown_gauge.set(current_drawdown);
        self.sharpe_gauge.set(self.sharpe_ratio);
        self.win_rate_gauge.set(self.win_rate);
        self.position_count_gauge.set(self.positions.len() as f64);
    }

    // Private helper methods
    fn update_portfolio_metrics(&mut self) {
        self.calculate_sharpe_ratio();
        self.calculate_win_rate();
        self.calculate_var_95();
        self.update_high_water_mark();
        self.update_max_drawdown();
        self.update_metrics();
    }

    fn calculate_invested_capital(&self) -> f64 {
        self.positions.values().map(|p| p.cost_basis.abs()).sum()
    }

    fn calculate_current_drawdown(&self) -> f64 {
        let current_value = self.portfolio_start_value + self.cumulative_pnl + self.unrealized_pnl;
        ((self.high_water_mark - current_value) / self.high_water_mark * 100.0).max(0.0)
    }

    fn calculate_sharpe_ratio(&mut self) {
        let returns = self.compute_return_series();

        if returns.is_empty() {
            self.sharpe_ratio = 0.0;
            return;
        }

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>()
            / returns.len() as f64;
        let std_dev = variance.sqrt();

        if std_dev > 0.0 {
            // Annualized Sharpe ratio (assuming daily returns)
            let excess_return = mean_return - (self.risk_free_rate / 365.0);
            self.sharpe_ratio = (excess_return / std_dev) * (365.0_f64).sqrt();
        } else {
            self.sharpe_ratio = 0.0;
        }
    }

    fn calculate_win_rate(&mut self) {
        let total_trades = self
            .strategy_pnl
            .values()
            .map(|s| s.trade_count)
            .sum::<u32>();

        let winning_trades = self.strategy_pnl.values().map(|s| s.win_count).sum::<u32>();

        if total_trades > 0 {
            self.win_rate = (winning_trades as f64 / total_trades as f64) * 100.0;
        } else {
            self.win_rate = 0.0;
        }
    }

    fn compute_return_series(&self) -> Vec<f64> {
        if self.pnl_history.len() < 2 {
            return Vec::new();
        }

        let history_vec: Vec<&PnLSnapshot> = self.pnl_history.iter().collect();
        history_vec
            .windows(2)
            .filter_map(|window| {
                let prev_value = window[0].portfolio_value;
                let curr_value = window[1].portfolio_value;
                if prev_value.abs() > f64::EPSILON {
                    Some((curr_value - prev_value) / prev_value)
                } else {
                    None
                }
            })
            .collect()
    }

    fn compute_benchmark_returns(&self) -> Vec<f64> {
        if self.intraday_pnl.len() < 2 {
            return Vec::new();
        }

        let mut points: Vec<_> = self.intraday_pnl.iter().collect();
        points.sort_by_key(|(ts, _)| *ts);

        let base = self.portfolio_start_value.abs().max(1.0);

        points
            .windows(2)
            .map(|window| {
                let prev = *window[0].1;
                let curr = *window[1].1;
                (curr - prev) / base
            })
            .collect()
    }

    fn calculate_sortino_ratio_from_returns(&self, returns: &[f64]) -> f64 {
        if returns.is_empty() {
            return 0.0;
        }

        let downside: Vec<f64> = returns.iter().copied().filter(|r| *r < 0.0).collect();

        if downside.is_empty() {
            return 0.0;
        }

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;
        let downside_variance =
            downside.iter().map(|r| r.powi(2)).sum::<f64>() / downside.len() as f64;

        if downside_variance <= 0.0 {
            return 0.0;
        }

        let downside_deviation = downside_variance.sqrt();
        let excess_return = mean_return - (self.risk_free_rate / 365.0);
        (excess_return / downside_deviation) * (365.0_f64).sqrt()
    }

    fn calculate_annualized_volatility(&self, returns: &[f64]) -> f64 {
        if returns.len() < 2 {
            return 0.0;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance = returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>()
            / (returns.len().saturating_sub(1)) as f64;

        if variance <= 0.0 {
            0.0
        } else {
            variance.sqrt() * (365.0_f64).sqrt()
        }
    }

    fn calculate_beta_from_returns(&self, returns: &[f64]) -> f64 {
        let benchmark = self.compute_benchmark_returns();
        let len = returns.len().min(benchmark.len());

        if len < 2 {
            return 0.0;
        }

        let returns_slice = &returns[returns.len() - len..];
        let benchmark_slice = &benchmark[benchmark.len() - len..];

        let mean_returns = returns_slice.iter().sum::<f64>() / len as f64;
        let mean_benchmark = benchmark_slice.iter().sum::<f64>() / len as f64;

        let covariance = returns_slice
            .iter()
            .zip(benchmark_slice.iter())
            .map(|(r, b)| (r - mean_returns) * (b - mean_benchmark))
            .sum::<f64>()
            / len as f64;

        let variance = benchmark_slice
            .iter()
            .map(|b| (b - mean_benchmark).powi(2))
            .sum::<f64>()
            / len as f64;

        if variance.abs() < 1e-9 {
            0.0
        } else {
            covariance / variance
        }
    }

    fn average_open_trade_duration(&self) -> f64 {
        if self.positions.is_empty() {
            return 0.0;
        }

        self.positions
            .values()
            .map(|p| p.duration_hours)
            .sum::<f64>()
            / self.positions.len() as f64
    }

    fn trades_per_day_over_history(&self, total_trades: u32) -> f64 {
        if total_trades == 0 {
            return 0.0;
        }

        let start = match self.pnl_history.front() {
            Some(snapshot) => snapshot.timestamp,
            None => return total_trades as f64,
        };

        let end = match self.pnl_history.back() {
            Some(snapshot) => snapshot.timestamp,
            None => return total_trades as f64,
        };

        let seconds = (end - start).num_seconds().max(0) as f64;
        let days = (seconds / 86_400.0).max(1.0 / 24.0);
        total_trades as f64 / days
    }

    fn calculate_var_95(&mut self) {
        if self.pnl_history.len() < 10 {
            self.var_95 = 0.0;
            return;
        }

        let history_vec: Vec<&PnLSnapshot> = self.pnl_history.iter().collect();
        let mut returns: Vec<f64> = history_vec
            .windows(2)
            .map(|window| window[1].total_pnl - window[0].total_pnl)
            .collect();

        returns.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let index = (returns.len() as f64 * 0.05) as usize;
        self.var_95 = returns.get(index).copied().unwrap_or(0.0).abs();
    }

    fn update_high_water_mark(&mut self) {
        let current_value = self.portfolio_start_value + self.cumulative_pnl + self.unrealized_pnl;
        if current_value > self.high_water_mark {
            self.high_water_mark = current_value;
        }
    }

    fn update_max_drawdown(&mut self) {
        let current_drawdown = self.calculate_current_drawdown();
        if current_drawdown > self.max_drawdown {
            self.max_drawdown = current_drawdown;
        }
    }

    fn calculate_risk_metrics(&self) -> RiskMetrics {
        let returns = self.compute_return_series();
        RiskMetrics {
            value_at_risk_95: self.var_95,
            max_drawdown: self.max_drawdown,
            current_drawdown: self.calculate_current_drawdown(),
            sharpe_ratio: self.sharpe_ratio,
            sortino_ratio: self.calculate_sortino_ratio_from_returns(&returns),
            calmar_ratio: if self.max_drawdown > 0.0 {
                (self.cumulative_pnl + self.unrealized_pnl) / self.max_drawdown
            } else {
                0.0
            },
            volatility: self.calculate_annualized_volatility(&returns),
            beta: self.calculate_beta_from_returns(&returns),
        }
    }

    fn calculate_performance_metrics(&self) -> PerformanceMetrics {
        let total_trades = self
            .strategy_pnl
            .values()
            .map(|s| s.trade_count)
            .sum::<u32>();
        let winning_trades = self.strategy_pnl.values().map(|s| s.win_count).sum::<u32>();
        let losing_trades = self
            .strategy_pnl
            .values()
            .map(|s| s.loss_count)
            .sum::<u32>();

        let total_wins: f64 = self
            .strategy_pnl
            .values()
            .filter(|s| s.best_trade > 0.0)
            .map(|s| s.best_trade)
            .sum();

        let total_losses: f64 = self
            .strategy_pnl
            .values()
            .filter(|s| s.worst_trade < 0.0)
            .map(|s| s.worst_trade.abs())
            .sum();

        PerformanceMetrics {
            total_trades,
            winning_trades,
            losing_trades,
            win_rate: self.win_rate,
            average_win: if winning_trades > 0 {
                total_wins / winning_trades as f64
            } else {
                0.0
            },
            average_loss: if losing_trades > 0 {
                total_losses / losing_trades as f64
            } else {
                0.0
            },
            profit_factor: if total_losses > 0.0 {
                total_wins / total_losses
            } else {
                0.0
            },
            largest_win: self
                .strategy_pnl
                .values()
                .map(|s| s.best_trade)
                .fold(0.0, f64::max),
            largest_loss: self
                .strategy_pnl
                .values()
                .map(|s| s.worst_trade)
                .fold(0.0, f64::min),
            average_trade_duration_hours: self.average_open_trade_duration(),
            trades_per_day: self.trades_per_day_over_history(total_trades),
        }
    }
}
