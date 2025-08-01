mod config;
mod database;
mod executor;
mod jito_client;
mod jupiter;
mod signer_client;
mod strategies;
mod risk_manager;
mod metrics;

use crate::config::CONFIG;
use anyhow::Result;
use database::Database;
use executor::MasterExecutor;
use risk_manager::RiskManager;
use metrics::MetricsServer;
use std::sync::Arc;
use tracing::{info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use axum::{routing::get, Router};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .init();

    info!("🚀 Starting MemeSnipe v25 Executor - Wintermute Grade");

    // Initialize database
    let db = Arc::new(Database::new(&CONFIG.database_url).await?);
    
    // Initialize risk manager
    let risk_manager = Arc::new(RiskManager::new(
        CONFIG.initial_capital_usd,
        CONFIG.max_daily_drawdown_percent,
        CONFIG.portfolio_stop_loss_percent,
    ));
    
    // Start metrics server
    let metrics_server = MetricsServer::new();
    let metrics_handle = tokio::spawn(async move {
        let app = Router::new()
            .route("/metrics", get(metrics_server.handle_metrics))
            .route("/health", get(|| async { "OK" }));
        
        let addr = SocketAddr::from(([0, 0, 0, 0], 9184));
        info!("Metrics server listening on {}", addr);
        
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    });

    // Initialize and run executor
    let mut master_executor = MasterExecutor::new(db.clone(), risk_manager.clone()).await?;
    
    // Run main execution loop
    tokio::select! {
        result = master_executor.run() => {
            if let Err(e) = result {
                tracing::error!("Executor failed: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    Ok(())
}
