use anyhow::{anyhow, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use shared_models::{SignRequest, SignResponse};
use solana_sdk::{
    signature::{read_keypair_file, Keypair, Signer},
    transaction::VersionedTransaction,
    message::VersionedMessage,
};
use std::{env, net::SocketAddr, sync::Arc};
use tracing::{error, info, instrument, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use base64::{Engine as _, engine::general_purpose};
use serde_json::json;
use tokio::sync::RwLock;
use zeroize::Zeroizing;

struct AppState {
    keypair: Arc<Keypair>,
    request_count: Arc<RwLock<u64>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .init();

    info!("🔐 Starting Signer Service v25 - Production Grade");

    let keypair_filename = env::var("WALLET_KEYPAIR_FILENAME")
        .map_err(|_| anyhow!("WALLET_KEYPAIR_FILENAME environment variable must be set"))?;
    let keypair_path = format!("/app/wallet/{}", keypair_filename);
    
    // Read keypair with secure memory handling
    let keypair_bytes = Zeroizing::new(
        std::fs::read(&keypair_path)
            .map_err(|e| anyhow!("Failed to read keypair at {}: {}", keypair_path, e))?
    );
    
    let keypair = Keypair::from_bytes(&keypair_bytes)
        .map_err(|e| anyhow!("Invalid keypair format: {}", e))?;
    
    let pubkey = keypair.pubkey();
    info!(%pubkey, "Wallet loaded successfully");

    let state = Arc::new(AppState {
        keypair: Arc::new(keypair),
        request_count: Arc::new(RwLock::new(0)),
    });

    let app = Router::new()
        .route("/pubkey", get(get_pubkey))
        .route("/sign", post(sign_transaction))
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8989));
    info!("Signer service listening on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[instrument(skip(state), name = "get_pubkey_handler")]
async fn get_pubkey(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(json!({ 
        "pubkey": state.keypair.pubkey().to_string(),
        "service": "signer",
        "version": "25.0.0"
    }))
}

#[instrument(skip(state, request), name = "sign_transaction_handler")]
async fn sign_transaction(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SignRequest>,
) -> Result<Json<SignResponse>, StatusCode> {
    // Increment request counter
    {
        let mut count = state.request_count.write().await;
        *count += 1;
    }
    
    // Decode transaction
    let tx_bytes = match general_purpose::STANDARD.decode(&request.transaction_b64) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!(error = %e, "Failed to decode base64 transaction");
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Deserialize transaction
    let mut tx: VersionedTransaction = match bincode::deserialize(&tx_bytes) {
        Ok(tx) => tx,
        Err(e) => {
            error!(error = %e, "Failed to deserialize transaction");
            return Err(StatusCode::BAD_REQUEST);
        }
    };
    
    // Validate transaction
    if tx.signatures.is_empty() {
        error!("Transaction has no signature slots");
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Get message bytes for signing
    let message_bytes = match &tx.message {
        VersionedMessage::Legacy(legacy) => legacy.serialize(),
        VersionedMessage::V0(v0) => {
            let writer = Vec::new();
            let mut cursor = std::io::Cursor::new(writer);
            
            // Manually serialize v0 message
            bincode::serialize_into(&mut cursor, &v0.header).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            bincode::serialize_into(&mut cursor, &v0.account_keys).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            bincode::serialize_into(&mut cursor, &v0.recent_blockhash).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            bincode::serialize_into(&mut cursor, &v0.instructions).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            bincode::serialize_into(&mut cursor, &v0.address_table_lookups).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            cursor.into_inner()
        }
    };

    // Sign the transaction
    let signature = state.keypair.sign_message(&message_bytes);
    tx.signatures[0] = signature;

    // Serialize signed transaction
    let signed_tx_bytes = match bincode::serialize(&tx) {
        Ok(bytes) => bytes,
        Err(e) => {
            error!(error = %e, "Failed to serialize signed transaction");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    info!("Transaction signed successfully");
    
    Ok(Json(SignResponse {
        signed_transaction_b64: general_purpose::STANDARD.encode(&signed_tx_bytes),
    }))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn get_metrics(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let count = *state.request_count.read().await;
    
    Json(json!({
        "signatures_created": count,
        "status": "healthy",
        "uptime_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }))
}
