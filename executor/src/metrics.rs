use prometheus::{Counter, Gauge, Histogram, HistogramOpts, Opts, Registry, Encoder, TextEncoder};
use shared_models::error::{Result, ModelError};
use axum::{http::StatusCode, response::Response, routing::get, Router};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tracing::{info, error, warn};

pub struct Metrics {
    pub trades_total: Counter,
    pub portfolio_value: Gauge,
    pub daily_pnl: Gauge,
    pub strategy_performance: Gauge,
    pub risk_events: Counter,
    pub execution_time: Histogram,
    pub order_latency: Histogram,
    pub slippage_histogram: Histogram,
    pub jupiter_quote_time: Histogram,
    pub mev_protection_time: Histogram,
    pub data_freshness: Gauge,
    registry: Registry,
}

impl Metrics {
    pub fn new(metrics_port: Option<u16>) -> Result<Arc<Self>> {
        let registry = Registry::new();

        let trades_total = Counter::with_opts(Opts::new(
            "trades_total",
            "Total number of trades executed"
        )).map_err(|e| ModelError::Config(format!("Failed to create trades_total metric: {}", e)))?;
        
        let portfolio_value = Gauge::with_opts(Opts::new(
            "portfolio_value_usd",
            "Current portfolio value in USD"
        )).map_err(|e| ModelError::Config(format!("Failed to create portfolio_value metric: {}", e)))?;
        
        let daily_pnl = Gauge::with_opts(Opts::new(
            "daily_pnl_usd", 
            "Daily profit and loss in USD"
        ))?;
        
        let strategy_performance = Gauge::with_opts(Opts::new(
            "strategy_performance_total",
            "Strategy performance total return"
        ))?;
        
        let risk_events = Counter::with_opts(Opts::new(
            "risk_events_total",
            "Total number of risk events"
        ))?;
        
        let execution_time = Histogram::with_opts(HistogramOpts::new(
            "trade_execution_seconds",
            "Time taken to execute trades"
        ))?;

        let order_latency = Histogram::with_opts(HistogramOpts::new(
            "order_latency_seconds",
            "Time from signal to order placement"
        ).buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0]))?;

        let slippage_histogram = Histogram::with_opts(HistogramOpts::new(
            "slippage_basis_points",
            "Slippage in basis points"
        ).buckets(vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]))?;

        let jupiter_quote_time = Histogram::with_opts(HistogramOpts::new(
            "jupiter_quote_time_seconds",
            "Time to get quote from Jupiter"
        ).buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]))?;

        let mev_protection_time = Histogram::with_opts(HistogramOpts::new(
            "mev_protection_time_seconds", 
            "Time for MEV protection processing"
        ).buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0]))?;

        let data_freshness = Gauge::with_opts(Opts::new(
            "data_freshness_seconds",
            "Age of latest market data in seconds"
        ))?;

        registry.register(Box::new(trades_total.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register trades_total: {}", e)))?;
        registry.register(Box::new(portfolio_value.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register portfolio_value: {}", e)))?;
        registry.register(Box::new(daily_pnl.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register daily_pnl: {}", e)))?;
        registry.register(Box::new(strategy_performance.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register strategy_performance: {}", e)))?;
        registry.register(Box::new(risk_events.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register risk_events: {}", e)))?;
        registry.register(Box::new(execution_time.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register execution_time: {}", e)))?;
        registry.register(Box::new(order_latency.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register order_latency: {}", e)))?;
        registry.register(Box::new(slippage_histogram.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register slippage_histogram: {}", e)))?;
        registry.register(Box::new(jupiter_quote_time.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register jupiter_quote_time: {}", e)))?;
        registry.register(Box::new(mev_protection_time.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register mev_protection_time: {}", e)))?;
        registry.register(Box::new(data_freshness.clone()))
            .map_err(|e| ModelError::Config(format!("Failed to register data_freshness: {}", e)))?;

        let metrics = Arc::new(Self {
            trades_total,
            portfolio_value,
            daily_pnl,
            strategy_performance,
            risk_events,
            execution_time,
            order_latency,
            slippage_histogram,
            jupiter_quote_time,
            mev_protection_time,
            data_freshness,
            registry,
        });

        // Start Prometheus HTTP server if port is specified
        if let Some(port) = metrics_port {
            let metrics_clone = metrics.clone();
            tokio::spawn(async move {
                if let Err(e) = start_metrics_server(port, metrics_clone).await {
                    error!("Failed to start metrics server: {}", e);
                }
            });
        }

        Ok(metrics)
    }

    pub fn get_registry(&self) -> &Registry {
        &self.registry
    }

    pub fn record_trade(&self) {
        self.trades_total.inc();
    }

    pub fn update_portfolio_value(&self, value: f64) {
        self.portfolio_value.set(value);
    }

    pub fn update_daily_pnl(&self, pnl: f64) {
        self.daily_pnl.set(pnl);
    }

    pub fn record_risk_event(&self) {
        self.risk_events.inc();
    }

    pub fn record_execution_time(&self, duration_seconds: f64) {
        self.execution_time.observe(duration_seconds);
    }

    pub fn record_order_latency(&self, duration_seconds: f64) {
        self.order_latency.observe(duration_seconds);
        
        // Warn if latency exceeds institutional standards
        if duration_seconds > 0.5 {
            warn!("High order latency detected: {:.3}s", duration_seconds);
        }
    }

    pub fn record_slippage(&self, slippage_bp: f64) {
        self.slippage_histogram.observe(slippage_bp);
        
        // Alert on excessive slippage
        if slippage_bp > 100.0 {
            warn!("High slippage detected: {:.1} bp", slippage_bp);
        }
    }

    pub fn record_jupiter_quote_time(&self, duration_seconds: f64) {
        self.jupiter_quote_time.observe(duration_seconds);
    }

    pub fn record_mev_protection_time(&self, duration_seconds: f64) {
        self.mev_protection_time.observe(duration_seconds);
    }

    pub fn update_data_freshness(&self, age_seconds: f64) {
        self.data_freshness.set(age_seconds);
        
        // Alert if data is stale (older than 500ms)
        if age_seconds > 0.5 {
            warn!("Stale market data detected: {:.3}s old", age_seconds);
        }
    }

    /// Create a timing context for comprehensive execution measurement
    pub fn start_execution_timer(&self) -> ExecutionTimer {
        ExecutionTimer::new(self)
    }
}

/// Comprehensive execution timing context
pub struct ExecutionTimer<'a> {
    metrics: &'a Metrics,
    start_time: Instant,
    signal_time: Option<Instant>,
    quote_start: Option<Instant>,
    mev_start: Option<Instant>,
}

impl<'a> ExecutionTimer<'a> {
    fn new(metrics: &'a Metrics) -> Self {
        Self {
            metrics,
            start_time: Instant::now(),
            signal_time: None,
            quote_start: None,
            mev_start: None,
        }
    }

    pub fn mark_signal_received(&mut self) {
        self.signal_time = Some(Instant::now());
    }

    pub fn mark_jupiter_quote_start(&mut self) {
        self.quote_start = Some(Instant::now());
    }

    pub fn mark_jupiter_quote_end(&mut self) {
        if let Some(start) = self.quote_start.take() {
            let duration = start.elapsed().as_secs_f64();
            self.metrics.record_jupiter_quote_time(duration);
        }
    }

    pub fn mark_mev_protection_start(&mut self) {
        self.mev_start = Some(Instant::now());
    }

    pub fn mark_mev_protection_end(&mut self) {
        if let Some(start) = self.mev_start.take() {
            let duration = start.elapsed().as_secs_f64();
            self.metrics.record_mev_protection_time(duration);
        }
    }

    pub fn complete_with_slippage(&self, slippage_bp: f64) {
        // Record total execution time
        let total_duration = self.start_time.elapsed().as_secs_f64();
        self.metrics.record_execution_time(total_duration);

        // Record order latency (signal to start)
        if let Some(signal_time) = self.signal_time {
            let latency = signal_time.elapsed().as_secs_f64();
            self.metrics.record_order_latency(latency);
        }

        // Record slippage
        self.metrics.record_slippage(slippage_bp);
    }
}

// Prometheus HTTP server for metrics export
async fn start_metrics_server(port: u16, metrics: Arc<Metrics>) -> Result<()> {
    info!("Starting Prometheus metrics server on port {}", port);
    
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await
        .map_err(|e| ModelError::Network(format!("Failed to bind metrics server: {}", e)))?;
        
    info!("Metrics server listening on http://0.0.0.0:{}/metrics", port);
    
    axum::serve(listener, app).await
        .map_err(|e| ModelError::Network(format!("Metrics server failed: {}", e)))?;
    
    Ok(())
}

async fn metrics_handler(
    axum::extract::State(metrics): axum::extract::State<Arc<Metrics>>,
) -> std::result::Result<Response<String>, StatusCode> {
    let encoder = TextEncoder::new();
    let metric_families = metrics.get_registry().gather();
    
    match encoder.encode_to_string(&metric_families) {
        Ok(output) => {
            Ok(Response::builder()
                .header("content-type", "text/plain; version=0.0.4")
                .body(output)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

pub struct MetricsServer {
    metrics: Arc<Metrics>,
}

impl MetricsServer {
    pub fn new() -> Result<Self> {
        let metrics = Metrics::new(None)?;
        Ok(Self { metrics })
    }
    
    pub async fn handle_metrics(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = self.metrics.get_registry().gather();
        encoder.encode_to_string(&metric_families).unwrap_or_default()
    }
}
