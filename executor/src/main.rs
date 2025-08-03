use executor::{
    config::get_config,
    event_loop::EventLoop,
    circuit_breaker::CircuitBreaker,
    metrics::Metrics,
};
use shared_models::error::Result;
use tracing::{info, error, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables from .env file if it exists
    dotenvy::dotenv().ok();
    
    // Initialize logging
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .init();

    info!("🚀 Starting MemeSnipe v25 Executor - Production Grade");

    // Load and validate configuration
    let config = get_config()?;
    info!("Configuration loaded successfully");

    // Initialize metrics
    let metrics = Metrics::new(config.metrics_port)?;
    info!("Metrics initialized on port {:?}", config.metrics_port);

    // Initialize Redis client for circuit breaker
    let redis_client = redis::Client::open(config.redis_url.clone())
        .map_err(|e| shared_models::error::ModelError::Redis(format!("Failed to create Redis client: {}", e)))?;

    // Initialize circuit breaker
    let circuit_breaker = Arc::new(CircuitBreaker::new(config.clone(), redis_client));
    info!("Circuit breaker initialized");

    // Initialize strategy registry using the helper function
    let mut strategy_registry = executor::strategy_registry::initialize_strategies();
    info!("Strategies registered: {}", strategy_registry.strategy_count());

    // Initialize event loop
    let mut event_loop = EventLoop::new(
        &config.redis_url,
        strategy_registry,
    )?;
    info!("Event loop initialized");

    // Initialize the event loop
    event_loop.initialize().await?;

    info!("🎯 MemeSnipe v25 Executor ready - Starting event processing");
    
    // Start the main event loop
    if let Err(e) = event_loop.run().await {
        error!("Event loop failed: {}", e);
        return Err(e);
    }

    Ok(())
}