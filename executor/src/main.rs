use executor::{
    circuit_breaker::CircuitBreaker,
    config::get_config,
    database::Database,
    executor::MasterExecutor,
    jupiter::{jupiter_base_url, with_jupiter_headers},
    metrics::Metrics,
    risk_manager::RiskManager,
};
use reqwest::Client;
use serde_json::json;
use shared_models::error::Result;
use std::sync::Arc;
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

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

    // Optional smoke test: validate env and external connectivity without starting services
    if std::env::var("SMOKE_TEST")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        return run_smoke_test(&config).await;
    }

    // Initialize metrics
    let metrics = Metrics::new(config.metrics_port)?;
    info!("Metrics initialized on port {:?}", config.metrics_port);

    // Prepare risk manager with metrics
    let mut risk_manager = RiskManager::from_config(&config);
    risk_manager.attach_metrics(Arc::clone(&metrics));
    let risk_manager = Arc::new(risk_manager);

    // Initialize database (mocked automatically in paper mode)
    let db = Arc::new(Database::new(&config.database_url).await.map_err(|e| {
        shared_models::error::ModelError::Config(format!("Database initialization failed: {}", e))
    })?);

    // Initialize circuit breaker (separate Redis connection)
    let breaker_redis = redis::Client::open(config.redis_url.clone()).map_err(|e| {
        shared_models::error::ModelError::Redis(format!("Failed to create Redis client: {}", e))
    })?;

    let breaker_handle = CircuitBreaker::new(config.clone(), breaker_redis);
    info!("Circuit breaker initialized");
    tokio::spawn(async move {
        if let Err(e) = breaker_handle.tick().await {
            error!("Circuit breaker loop exited: {}", e);
        }
    });

    // Initialize master executor pipeline
    let mut master_executor = MasterExecutor::new(Arc::clone(&db), Arc::clone(&risk_manager))
        .await
        .map_err(|e| {
            shared_models::error::ModelError::Config(format!(
                "Failed to initialize master executor: {}",
                e
            ))
        })?;

    info!("🎯 MemeSnipe v25 Executor ready - Starting master execution loop");

    master_executor.run().await.map_err(|e| {
        shared_models::error::ModelError::Strategy(format!("Master executor runtime failed: {}", e))
    })?;

    Ok(())
}

async fn run_smoke_test(config: &executor::config::Config) -> Result<()> {
    use std::time::Duration;
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| {
            shared_models::error::ModelError::Network(format!("Failed to build HTTP client: {}", e))
        })?;

    // Presence check (no values printed)
    let required = vec![
        ("DATABASE_URL", !config.database_url.is_empty()),
        ("REDIS_URL", !config.redis_url.is_empty()),
        ("SOLANA_RPC_URL", !config.solana_rpc_url.is_empty()),
        ("SOLANA_WS_URL", !config.solana_ws_url.is_empty()),
        ("HELIUS_API_KEY", !config.helius_api_key.is_empty()),
        (
            "TWITTER_BEARER_TOKEN",
            !config.twitter_bearer_token.is_empty(),
        ),
        ("SIGNER_URL", !config.signer_url.is_empty()),
    ];
    let mut missing = Vec::new();
    for (k, ok) in &required {
        if *ok {
            info!(%k, "env: OK");
        } else {
            missing.push(*k);
        }
    }
    if !missing.is_empty() {
        info!(missing = ?missing, "env: missing required variables");
    }

    // Jupiter quote ping (no secrets required)
    let jup_url = jupiter_base_url();
    let jup_request = client.get(format!("{}/quote", jup_url)).query(&[
        ("inputMint", "So11111111111111111111111111111111111111112"),
        ("outputMint", "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
        ("amount", "1000000"),
        ("slippageBps", "50"),
    ]);
    let jup_status = with_jupiter_headers(jup_request)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    info!(ok = jup_status, "ping: jupiter_quote");

    // Jito engine ping (public)
    let jito_url = std::env::var("JITO_BLOCK_ENGINE_URL")
        .unwrap_or_else(|_| "https://mainnet.block-engine.jito.wtf/api/v1".to_string());
    let jito_status = client
        .get(jito_url.clone())
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    info!(ok = jito_status, "ping: jito_engine");

    // Solana RPC health (may require API key embedded in URL; URL never logged)
    let sol_ok = client
        .post(config.solana_rpc_url.clone())
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"getHealth"}))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    info!(ok = sol_ok, "ping: solana_rpc_health");

    // DexScreener token probe (public, no key)
    let ds_ok = client
        .get("https://api.dexscreener.com/latest/dex/tokens/So11111111111111111111111111111111111111112")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);
    info!(ok = ds_ok, "ping: dexscreener");

    // Exit with error if critical pings fail (require Solana only)
    if !sol_ok {
        return Err(
            shared_models::error::ModelError::Network("Smoke test failed: solana".into()).into(),
        );
    }

    info!("✅ Smoke test complete");
    Ok(())
}
