use axum::{http::StatusCode, response::Json, routing::get, Router};
use serde_json::{json, Value};
use shared_models::error::{Result, ModelError};
use std::sync::Arc;
use tracing::{error, debug};
use redis::Client;
use sqlx::{PgPool, Row};

pub struct HealthChecker {
    redis_client: Client,
    db_pool: Option<PgPool>,
}

impl HealthChecker {
    pub fn new(redis_client: Client, db_pool: Option<PgPool>) -> Self {
        Self {
            redis_client,
            db_pool,
        }
    }

    pub async fn check_redis(&self) -> Result<bool> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await
            .map_err(|e| ModelError::Redis(format!("Failed to connect to Redis: {}", e)))?;
        
        // Simple ping test
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| ModelError::Redis(format!("Redis PING failed: {}", e)))?;
        
        Ok(true)
    }

    pub async fn check_database(&self) -> Result<bool> {
        if let Some(pool) = &self.db_pool {
            let row = sqlx::query("SELECT 1 as test")
                .fetch_one(pool)
                .await
                .map_err(ModelError::Db)?;
            
            let test_value: i32 = row.try_get("test")
                .map_err(ModelError::Db)?;
            
            Ok(test_value == 1)
        } else {
            Ok(false) // No database configured
        }
    }

    pub async fn get_health_status(&self) -> Value {
        let mut status = json!({
            "service": "executor",
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "checks": {}
        });

        // Check Redis
        let redis_healthy = match self.check_redis().await {
            Ok(true) => {
                status["checks"]["redis"] = json!({"status": "healthy", "message": "Connected"});
                true
            }
            Ok(false) | Err(_) => {
                status["checks"]["redis"] = json!({"status": "unhealthy", "message": "Connection failed"});
                false
            }
        };

        // Check Database
        let db_healthy = match self.check_database().await {
            Ok(true) => {
                status["checks"]["database"] = json!({"status": "healthy", "message": "Connected"});
                true
            }
            Ok(false) => {
                status["checks"]["database"] = json!({"status": "not_configured", "message": "Database not configured"});
                true // Not configured is considered healthy
            }
            Err(e) => {
                status["checks"]["database"] = json!({"status": "unhealthy", "message": format!("Connection failed: {}", e)});
                false
            }
        };

        // Overall status
        if redis_healthy && db_healthy {
            status["status"] = json!("healthy");
        } else {
            status["status"] = json!("unhealthy");
        }

        status
    }
}

pub async fn health_handler(
    axum::extract::State(health_checker): axum::extract::State<Arc<HealthChecker>>,
) -> Result<Json<Value>, StatusCode> {
    let status = health_checker.get_health_status().await;
    
    if status["status"] == "healthy" {
        Ok(Json(status))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub async fn readiness_handler(
    axum::extract::State(health_checker): axum::extract::State<Arc<HealthChecker>>,
) -> Result<Json<Value>, StatusCode> {
    // For readiness, we check if all critical services are available
    let redis_ok = health_checker.check_redis().await.unwrap_or(false);
    
    let status = json!({
        "service": "executor",
        "ready": redis_ok,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "checks": {
            "redis": {"ready": redis_ok}
        }
    });

    if redis_ok {
        Ok(Json(status))
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub fn create_health_router(health_checker: Arc<HealthChecker>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(readiness_handler))
        .with_state(health_checker)
}
