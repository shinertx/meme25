use serde::{Deserialize, Serialize};
use shared_models::error::{ModelError, Result};
use std::collections::HashMap;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionConfig {
    pub deployment: DeploymentConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub security: SecurityConfig,
    pub monitoring: MonitoringConfig,
    pub performance: PerformanceConfig,
    pub logging: LoggingConfig,
    pub networking: NetworkingConfig,
    pub backup: BackupConfig,
    pub alerts: AlertConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentConfig {
    pub environment: String, // "production", "staging", "development"
    pub version: String,
    pub release_date: String,
    pub replicas: u32,
    pub health_check_path: String,
    pub graceful_shutdown_timeout_seconds: u32,
    pub max_memory_mb: u32,
    pub max_cpu_cores: f32,
    pub restart_policy: String,      // "Always", "OnFailure", "Never"
    pub deployment_strategy: String, // "RollingUpdate", "BlueGreen", "Canary"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password_env_var: String,
    pub ssl_mode: String, // "require", "prefer", "disable"
    pub max_connections: u32,
    pub connection_timeout_seconds: u32,
    pub query_timeout_seconds: u32,
    pub backup_retention_days: u32,
    pub read_replicas: Vec<String>,
    pub migration_on_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password_env_var: String,
    pub ssl_enabled: bool,
    pub max_connections: u32,
    pub connection_timeout_seconds: u32,
    pub cluster_mode: bool,
    pub cluster_nodes: Vec<String>,
    pub sentinel_mode: bool,
    pub sentinel_master_name: String,
    pub key_prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub jwt_secret_env_var: String,
    pub api_key_env_var: String,
    pub rpc_endpoint_env_var: String,
    pub private_key_env_var: String,
    pub rate_limit_requests_per_minute: u32,
    pub cors_allowed_origins: Vec<String>,
    pub tls_cert_path: String,
    pub tls_key_path: String,
    pub oauth_providers: HashMap<String, String>,
    pub ip_whitelist: Vec<String>,
    pub enable_request_signing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub prometheus_port: u16,
    pub prometheus_path: String,
    pub jaeger_endpoint: String,
    pub health_check_port: u16,
    pub metrics_retention_days: u32,
    pub alert_manager_url: String,
    pub grafana_dashboard_url: String,
    pub sentry_dsn_env_var: String,
    pub enable_distributed_tracing: bool,
    pub custom_metrics: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub worker_threads: u32,
    pub async_runtime_blocking_threads: u32,
    pub request_buffer_size: u32,
    pub websocket_buffer_size: u32,
    pub cache_size_mb: u32,
    pub cache_ttl_seconds: u32,
    pub enable_compression: bool,
    pub enable_http2: bool,
    pub connection_pool_size: u32,
    pub keep_alive_timeout_seconds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,  // "trace", "debug", "info", "warn", "error"
    pub format: String, // "json", "pretty", "compact"
    pub output: String, // "stdout", "file", "both"
    pub file_path: String,
    pub max_file_size_mb: u32,
    pub max_files: u32,
    pub retention_days: u32,
    pub enable_structured_logging: bool,
    pub enable_sampling: bool,
    pub sampling_rate: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkingConfig {
    pub http_port: u16,
    pub websocket_port: u16,
    pub grpc_port: u16,
    pub admin_port: u16,
    pub bind_address: String,
    pub external_hostname: String,
    pub load_balancer_url: String,
    pub cdn_url: String,
    pub timeout_seconds: u32,
    pub max_request_size_mb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub enabled: bool,
    pub backup_schedule: String, // Cron format
    pub backup_retention_days: u32,
    pub backup_storage_path: String,
    pub backup_compression: bool,
    pub backup_encryption: bool,
    pub s3_bucket: Option<String>,
    pub s3_region: Option<String>,
    pub s3_access_key_env_var: Option<String>,
    pub s3_secret_key_env_var: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: String,
    pub smtp_password_env_var: String,
    pub smtp_from_address: String,
    pub alert_recipients: Vec<String>,
    pub slack_webhook_env_var: String,
    pub discord_webhook_env_var: String,
    pub telegram_bot_token_env_var: String,
    pub telegram_chat_id: String,
    pub enable_sms: bool,
    pub sms_provider: String,
    pub sms_api_key_env_var: String,
}

impl ProductionConfig {
    pub fn load() -> Result<Self> {
        let environment =
            std::env::var("DEPLOYMENT_ENV").unwrap_or_else(|_| "development".to_string());

        match environment.as_str() {
            "production" => Self::production_config(),
            "staging" => Self::staging_config(),
            "development" => Self::development_config(),
            _ => {
                warn!(
                    "Unknown environment '{}', using development config",
                    environment
                );
                Self::development_config()
            }
        }
    }

    pub fn production_config() -> Result<Self> {
        info!("Loading production configuration");

        Ok(ProductionConfig {
            deployment: DeploymentConfig {
                environment: "production".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                release_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                replicas: 3,
                health_check_path: "/health".to_string(),
                graceful_shutdown_timeout_seconds: 30,
                max_memory_mb: 2048,
                max_cpu_cores: 2.0,
                restart_policy: "Always".to_string(),
                deployment_strategy: "RollingUpdate".to_string(),
            },
            database: DatabaseConfig {
                host: std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: std::env::var("DB_PORT")
                    .unwrap_or_else(|_| "5432".to_string())
                    .parse()
                    .unwrap_or(5432),
                database: std::env::var("DB_NAME")
                    .unwrap_or_else(|_| "meme_snipe_prod".to_string()),
                username: std::env::var("DB_USER").unwrap_or_else(|_| "meme_snipe".to_string()),
                password_env_var: "DB_PASSWORD".to_string(),
                ssl_mode: "require".to_string(),
                max_connections: 50,
                connection_timeout_seconds: 30,
                query_timeout_seconds: 60,
                backup_retention_days: 30,
                read_replicas: vec![
                    std::env::var("DB_READ_REPLICA_1").unwrap_or_default(),
                    std::env::var("DB_READ_REPLICA_2").unwrap_or_default(),
                ]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect(),
                migration_on_startup: false,
            },
            redis: RedisConfig {
                host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: std::env::var("REDIS_PORT")
                    .unwrap_or_else(|_| "6379".to_string())
                    .parse()
                    .unwrap_or(6379),
                password_env_var: "REDIS_PASSWORD".to_string(),
                ssl_enabled: true,
                max_connections: 100,
                connection_timeout_seconds: 10,
                cluster_mode: std::env::var("REDIS_CLUSTER")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                cluster_nodes: std::env::var("REDIS_CLUSTER_NODES")
                    .unwrap_or_default()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                sentinel_mode: false,
                sentinel_master_name: "mymaster".to_string(),
                key_prefix: "meme_snipe:prod:".to_string(),
            },
            security: SecurityConfig {
                jwt_secret_env_var: "JWT_SECRET".to_string(),
                api_key_env_var: "API_KEY".to_string(),
                rpc_endpoint_env_var: "SOLANA_RPC_URL".to_string(),
                private_key_env_var: "SOLANA_PRIVATE_KEY".to_string(),
                rate_limit_requests_per_minute: 1000,
                cors_allowed_origins: vec![
                    "https://dashboard.memesnipe.com".to_string(),
                    "https://api.memesnipe.com".to_string(),
                ],
                tls_cert_path: "/etc/ssl/certs/memesnipe.crt".to_string(),
                tls_key_path: "/etc/ssl/private/memesnipe.key".to_string(),
                oauth_providers: HashMap::from([
                    (
                        "google".to_string(),
                        std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
                    ),
                    (
                        "github".to_string(),
                        std::env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
                    ),
                ]),
                ip_whitelist: std::env::var("IP_WHITELIST")
                    .unwrap_or_default()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                enable_request_signing: true,
            },
            monitoring: MonitoringConfig {
                prometheus_port: 9090,
                prometheus_path: "/metrics".to_string(),
                jaeger_endpoint: std::env::var("JAEGER_ENDPOINT")
                    .unwrap_or_else(|_| "http://jaeger:14268/api/traces".to_string()),
                health_check_port: 8080,
                metrics_retention_days: 90,
                alert_manager_url: std::env::var("ALERTMANAGER_URL")
                    .unwrap_or_else(|_| "http://alertmanager:9093".to_string()),
                grafana_dashboard_url: std::env::var("GRAFANA_URL")
                    .unwrap_or_else(|_| "https://grafana.memesnipe.com".to_string()),
                sentry_dsn_env_var: "SENTRY_DSN".to_string(),
                enable_distributed_tracing: true,
                custom_metrics: HashMap::from([
                    ("business_metrics".to_string(), "enabled".to_string()),
                    ("trading_metrics".to_string(), "enabled".to_string()),
                ]),
            },
            performance: PerformanceConfig {
                worker_threads: num_cpus::get() as u32,
                async_runtime_blocking_threads: 512,
                request_buffer_size: 8192,
                websocket_buffer_size: 4096,
                cache_size_mb: 512,
                cache_ttl_seconds: 300,
                enable_compression: true,
                enable_http2: true,
                connection_pool_size: 50,
                keep_alive_timeout_seconds: 60,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                output: "both".to_string(),
                file_path: "/var/log/memesnipe/executor.log".to_string(),
                max_file_size_mb: 100,
                max_files: 10,
                retention_days: 30,
                enable_structured_logging: true,
                enable_sampling: false,
                sampling_rate: 1.0,
            },
            networking: NetworkingConfig {
                http_port: 8000,
                websocket_port: 8001,
                grpc_port: 8002,
                admin_port: 8003,
                bind_address: "0.0.0.0".to_string(),
                external_hostname: std::env::var("EXTERNAL_HOSTNAME")
                    .unwrap_or_else(|_| "api.memesnipe.com".to_string()),
                load_balancer_url: std::env::var("LOAD_BALANCER_URL")
                    .unwrap_or_else(|_| "https://lb.memesnipe.com".to_string()),
                cdn_url: std::env::var("CDN_URL")
                    .unwrap_or_else(|_| "https://cdn.memesnipe.com".to_string()),
                timeout_seconds: 30,
                max_request_size_mb: 10,
            },
            backup: BackupConfig {
                enabled: true,
                backup_schedule: "0 2 * * *".to_string(), // Daily at 2 AM UTC
                backup_retention_days: 90,
                backup_storage_path: "/var/backups/memesnipe".to_string(),
                backup_compression: true,
                backup_encryption: true,
                s3_bucket: std::env::var("BACKUP_S3_BUCKET").ok(),
                s3_region: std::env::var("BACKUP_S3_REGION").ok(),
                s3_access_key_env_var: Some("BACKUP_S3_ACCESS_KEY".to_string()),
                s3_secret_key_env_var: Some("BACKUP_S3_SECRET_KEY".to_string()),
            },
            alerts: AlertConfig {
                smtp_host: std::env::var("SMTP_HOST")
                    .unwrap_or_else(|_| "smtp.gmail.com".to_string()),
                smtp_port: std::env::var("SMTP_PORT")
                    .unwrap_or_else(|_| "587".to_string())
                    .parse()
                    .unwrap_or(587),
                smtp_username: std::env::var("SMTP_USERNAME").unwrap_or_default(),
                smtp_password_env_var: "SMTP_PASSWORD".to_string(),
                smtp_from_address: std::env::var("SMTP_FROM")
                    .unwrap_or_else(|_| "alerts@memesnipe.com".to_string()),
                alert_recipients: std::env::var("ALERT_RECIPIENTS")
                    .unwrap_or_default()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                slack_webhook_env_var: "SLACK_WEBHOOK_URL".to_string(),
                discord_webhook_env_var: "DISCORD_WEBHOOK_URL".to_string(),
                telegram_bot_token_env_var: "TELEGRAM_BOT_TOKEN".to_string(),
                telegram_chat_id: std::env::var("TELEGRAM_CHAT_ID").unwrap_or_default(),
                enable_sms: std::env::var("ENABLE_SMS")
                    .unwrap_or_else(|_| "false".to_string())
                    .parse()
                    .unwrap_or(false),
                sms_provider: "twilio".to_string(),
                sms_api_key_env_var: "TWILIO_API_KEY".to_string(),
            },
        })
    }

    pub fn staging_config() -> Result<Self> {
        info!("Loading staging configuration");

        let mut config = Self::production_config()?;

        // Override staging-specific settings
        config.deployment.environment = "staging".to_string();
        config.deployment.replicas = 2;
        config.database.database = "meme_snipe_staging".to_string();
        config.database.backup_retention_days = 7;
        config.redis.key_prefix = "meme_snipe:staging:".to_string();
        config.security.cors_allowed_origins = vec![
            "https://staging-dashboard.memesnipe.com".to_string(),
            "https://staging-api.memesnipe.com".to_string(),
        ];
        config.monitoring.metrics_retention_days = 30;
        config.logging.level = "debug".to_string();
        config.backup.backup_retention_days = 30;

        Ok(config)
    }

    pub fn development_config() -> Result<Self> {
        info!("Loading development configuration");

        Ok(ProductionConfig {
            deployment: DeploymentConfig {
                environment: "development".to_string(),
                version: "dev".to_string(),
                release_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
                replicas: 1,
                health_check_path: "/health".to_string(),
                graceful_shutdown_timeout_seconds: 10,
                max_memory_mb: 512,
                max_cpu_cores: 1.0,
                restart_policy: "OnFailure".to_string(),
                deployment_strategy: "RollingUpdate".to_string(),
            },
            database: DatabaseConfig {
                host: "localhost".to_string(),
                port: 5432,
                database: "meme_snipe_dev".to_string(),
                username: "meme_snipe".to_string(),
                password_env_var: "DB_PASSWORD".to_string(),
                ssl_mode: "prefer".to_string(),
                max_connections: 10,
                connection_timeout_seconds: 10,
                query_timeout_seconds: 30,
                backup_retention_days: 3,
                read_replicas: vec![],
                migration_on_startup: true,
            },
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                password_env_var: "REDIS_PASSWORD".to_string(),
                ssl_enabled: false,
                max_connections: 10,
                connection_timeout_seconds: 5,
                cluster_mode: false,
                cluster_nodes: vec![],
                sentinel_mode: false,
                sentinel_master_name: "mymaster".to_string(),
                key_prefix: "meme_snipe:dev:".to_string(),
            },
            security: SecurityConfig {
                jwt_secret_env_var: "JWT_SECRET".to_string(),
                api_key_env_var: "API_KEY".to_string(),
                rpc_endpoint_env_var: "SOLANA_RPC_URL".to_string(),
                private_key_env_var: "SOLANA_PRIVATE_KEY".to_string(),
                rate_limit_requests_per_minute: 100,
                cors_allowed_origins: vec![
                    "http://localhost:3000".to_string(),
                    "http://127.0.0.1:3000".to_string(),
                ],
                tls_cert_path: "".to_string(),
                tls_key_path: "".to_string(),
                oauth_providers: HashMap::new(),
                ip_whitelist: vec![],
                enable_request_signing: false,
            },
            monitoring: MonitoringConfig {
                prometheus_port: 9090,
                prometheus_path: "/metrics".to_string(),
                jaeger_endpoint: "http://localhost:14268/api/traces".to_string(),
                health_check_port: 8080,
                metrics_retention_days: 7,
                alert_manager_url: "http://localhost:9093".to_string(),
                grafana_dashboard_url: "http://localhost:3001".to_string(),
                sentry_dsn_env_var: "SENTRY_DSN".to_string(),
                enable_distributed_tracing: false,
                custom_metrics: HashMap::new(),
            },
            performance: PerformanceConfig {
                worker_threads: 2,
                async_runtime_blocking_threads: 32,
                request_buffer_size: 1024,
                websocket_buffer_size: 512,
                cache_size_mb: 64,
                cache_ttl_seconds: 60,
                enable_compression: false,
                enable_http2: false,
                connection_pool_size: 5,
                keep_alive_timeout_seconds: 30,
            },
            logging: LoggingConfig {
                level: "debug".to_string(),
                format: "pretty".to_string(),
                output: "stdout".to_string(),
                file_path: "./logs/executor.log".to_string(),
                max_file_size_mb: 10,
                max_files: 3,
                retention_days: 3,
                enable_structured_logging: false,
                enable_sampling: false,
                sampling_rate: 1.0,
            },
            networking: NetworkingConfig {
                http_port: 8000,
                websocket_port: 8001,
                grpc_port: 8002,
                admin_port: 8003,
                bind_address: "127.0.0.1".to_string(),
                external_hostname: "localhost".to_string(),
                load_balancer_url: "http://localhost:8000".to_string(),
                cdn_url: "http://localhost:8000".to_string(),
                timeout_seconds: 30,
                max_request_size_mb: 1,
            },
            backup: BackupConfig {
                enabled: false,
                backup_schedule: "0 6 * * *".to_string(),
                backup_retention_days: 3,
                backup_storage_path: "./backups".to_string(),
                backup_compression: false,
                backup_encryption: false,
                s3_bucket: None,
                s3_region: None,
                s3_access_key_env_var: None,
                s3_secret_key_env_var: None,
            },
            alerts: AlertConfig {
                smtp_host: "localhost".to_string(),
                smtp_port: 1025, // mailhog
                smtp_username: "".to_string(),
                smtp_password_env_var: "SMTP_PASSWORD".to_string(),
                smtp_from_address: "dev@memesnipe.local".to_string(),
                alert_recipients: vec!["dev@memesnipe.local".to_string()],
                slack_webhook_env_var: "SLACK_WEBHOOK_URL".to_string(),
                discord_webhook_env_var: "DISCORD_WEBHOOK_URL".to_string(),
                telegram_bot_token_env_var: "TELEGRAM_BOT_TOKEN".to_string(),
                telegram_chat_id: "".to_string(),
                enable_sms: false,
                sms_provider: "".to_string(),
                sms_api_key_env_var: "".to_string(),
            },
        })
    }

    pub fn validate(&self) -> Result<()> {
        info!("Validating production configuration");

        // Validate required environment variables exist
        let required_env_vars = match self.deployment.environment.as_str() {
            "production" => vec![
                &self.database.password_env_var,
                &self.redis.password_env_var,
                &self.security.jwt_secret_env_var,
                &self.security.api_key_env_var,
                &self.security.rpc_endpoint_env_var,
                &self.security.private_key_env_var,
            ],
            "staging" => vec![
                &self.database.password_env_var,
                &self.security.jwt_secret_env_var,
                &self.security.rpc_endpoint_env_var,
            ],
            _ => vec![], // Development mode is more lenient
        };

        for env_var in required_env_vars {
            if std::env::var(env_var).is_err() {
                error!("Required environment variable '{}' is not set", env_var);
                return Err(ModelError::Config(format!(
                    "Missing required environment variable: {}",
                    env_var
                )));
            }
        }

        // Validate configuration values
        if self.deployment.replicas == 0 {
            return Err(ModelError::Config(
                "Deployment replicas must be greater than 0".to_string(),
            ));
        }

        if self.database.max_connections == 0 {
            return Err(ModelError::Config(
                "Database max_connections must be greater than 0".to_string(),
            ));
        }

        if self.networking.http_port == 0 {
            return Err(ModelError::Config(
                "HTTP port must be greater than 0".to_string(),
            ));
        }

        // Validate network ports don't conflict
        let ports = vec![
            self.networking.http_port,
            self.networking.websocket_port,
            self.networking.grpc_port,
            self.networking.admin_port,
            self.monitoring.prometheus_port,
            self.monitoring.health_check_port,
        ];

        let mut unique_ports = std::collections::HashSet::new();
        for port in ports {
            if !unique_ports.insert(port) {
                return Err(ModelError::Config(format!(
                    "Port {} is used by multiple services",
                    port
                )));
            }
        }

        info!("Production configuration validation passed");
        Ok(())
    }

    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn generate_docker_compose(&self) -> Result<String> {
        let compose = format!(
            r#"version: '3.8'

services:
  executor:
    image: memesnipe/executor:{version}
    container_name: memesnipe_executor
    restart: {restart_policy}
    environment:
      - DEPLOYMENT_ENV={environment}
      - DB_HOST={db_host}
      - DB_PORT={db_port}
      - DB_NAME={db_name}
      - DB_USER={db_user}
      - REDIS_HOST={redis_host}
      - REDIS_PORT={redis_port}
      - EXTERNAL_HOSTNAME={external_hostname}
    ports:
      - "{http_port}:{http_port}"
      - "{websocket_port}:{websocket_port}"
      - "{prometheus_port}:{prometheus_port}"
      - "{health_port}:{health_port}"
    volumes:
      - ./logs:/var/log/memesnipe
      - ./backups:/var/backups/memesnipe
      - ./ssl:/etc/ssl/certs:ro
    depends_on:
      - postgres
      - redis
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:{health_port}/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    deploy:
      replicas: {replicas}
      resources:
        limits:
          memory: {max_memory}M
          cpus: '{max_cpu}'
        reservations:
          memory: {min_memory}M
          cpus: '{min_cpu}'

  postgres:
    image: postgres:15
    container_name: memesnipe_postgres
    restart: always
    environment:
      - POSTGRES_DB={db_name}
      - POSTGRES_USER={db_user}
      - POSTGRES_PASSWORD_FILE=/run/secrets/db_password
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./backups:/backups
    ports:
      - "{db_port}:{db_port}"
    secrets:
      - db_password

  redis:
    image: redis:7-alpine
    container_name: memesnipe_redis
    restart: always
    command: redis-server --requirepass-file /run/secrets/redis_password
    volumes:
      - redis_data:/data
    ports:
      - "{redis_port}:{redis_port}"
    secrets:
      - redis_password

  prometheus:
    image: prom/prometheus:latest
    container_name: memesnipe_prometheus
    restart: always
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus

  grafana:
    image: grafana/grafana:latest
    container_name: memesnipe_grafana
    restart: always
    ports:
      - "3001:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana:/etc/grafana/provisioning

volumes:
  postgres_data:
  redis_data:
  prometheus_data:
  grafana_data:

secrets:
  db_password:
    file: ./secrets/db_password.txt
  redis_password:
    file: ./secrets/redis_password.txt

networks:
  default:
    driver: bridge
"#,
            version = self.deployment.version,
            restart_policy = self.deployment.restart_policy.to_lowercase(),
            environment = self.deployment.environment,
            db_host = self.database.host,
            db_port = self.database.port,
            db_name = self.database.database,
            db_user = self.database.username,
            redis_host = self.redis.host,
            redis_port = self.redis.port,
            external_hostname = self.networking.external_hostname,
            http_port = self.networking.http_port,
            websocket_port = self.networking.websocket_port,
            prometheus_port = self.monitoring.prometheus_port,
            health_port = self.monitoring.health_check_port,
            replicas = self.deployment.replicas,
            max_memory = self.deployment.max_memory_mb,
            max_cpu = self.deployment.max_cpu_cores,
            min_memory = self.deployment.max_memory_mb / 2,
            min_cpu = self.deployment.max_cpu_cores / 2.0,
        );

        Ok(compose)
    }

    pub fn generate_kubernetes_manifests(&self) -> Result<String> {
        let manifests = format!(
            r#"apiVersion: apps/v1
kind: Deployment
metadata:
  name: memesnipe-executor
  namespace: memesnipe
spec:
  replicas: {replicas}
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: memesnipe-executor
  template:
    metadata:
      labels:
        app: memesnipe-executor
        version: {version}
    spec:
      containers:
      - name: executor
        image: memesnipe/executor:{version}
        ports:
        - containerPort: {http_port}
          name: http
        - containerPort: {websocket_port}
          name: websocket
        - containerPort: {prometheus_port}
          name: metrics
        - containerPort: {health_port}
          name: health
        env:
        - name: DEPLOYMENT_ENV
          value: "{environment}"
        - name: DB_HOST
          value: "{db_host}"
        - name: DB_PORT
          value: "{db_port}"
        - name: DB_NAME
          value: "{db_name}"
        - name: DB_USER
          value: "{db_user}"
        - name: DB_PASSWORD
          valueFrom:
            secretKeyRef:
              name: memesnipe-secrets
              key: db-password
        - name: REDIS_HOST
          value: "{redis_host}"
        - name: REDIS_PORT
          value: "{redis_port}"
        - name: REDIS_PASSWORD
          valueFrom:
            secretKeyRef:
              name: memesnipe-secrets
              key: redis-password
        resources:
          requests:
            memory: "{min_memory}Mi"
            cpu: "{min_cpu}"
          limits:
            memory: "{max_memory}Mi"
            cpu: "{max_cpu}"
        livenessProbe:
          httpGet:
            path: /health
            port: {health_port}
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health
            port: {health_port}
          initialDelaySeconds: 5
          periodSeconds: 5
        volumeMounts:
        - name: logs
          mountPath: /var/log/memesnipe
        - name: ssl-certs
          mountPath: /etc/ssl/certs
          readOnly: true
      volumes:
      - name: logs
        persistentVolumeClaim:
          claimName: memesnipe-logs
      - name: ssl-certs
        secret:
          secretName: memesnipe-tls

---
apiVersion: v1
kind: Service
metadata:
  name: memesnipe-executor-service
  namespace: memesnipe
spec:
  selector:
    app: memesnipe-executor
  ports:
  - name: http
    port: 80
    targetPort: {http_port}
  - name: websocket
    port: 8001
    targetPort: {websocket_port}
  - name: metrics
    port: 9090
    targetPort: {prometheus_port}
  type: ClusterIP

---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: memesnipe-executor-ingress
  namespace: memesnipe
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
spec:
  tls:
  - hosts:
    - {external_hostname}
    secretName: memesnipe-tls
  rules:
  - host: {external_hostname}
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: memesnipe-executor-service
            port:
              number: 80

---
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: memesnipe-logs
  namespace: memesnipe
spec:
  accessModes:
  - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
"#,
            replicas = self.deployment.replicas,
            version = self.deployment.version,
            http_port = self.networking.http_port,
            websocket_port = self.networking.websocket_port,
            prometheus_port = self.monitoring.prometheus_port,
            health_port = self.monitoring.health_check_port,
            environment = self.deployment.environment,
            db_host = self.database.host,
            db_port = self.database.port,
            db_name = self.database.database,
            db_user = self.database.username,
            redis_host = self.redis.host,
            redis_port = self.redis.port,
            max_memory = self.deployment.max_memory_mb,
            max_cpu = self.deployment.max_cpu_cores,
            min_memory = self.deployment.max_memory_mb / 2,
            min_cpu = self.deployment.max_cpu_cores / 2.0,
            external_hostname = self.networking.external_hostname,
        );

        Ok(manifests)
    }

    pub fn generate_systemd_service(&self) -> Result<String> {
        let service = format!(
            r#"[Unit]
Description=MemeSnipe v25 Executor
After=network.target postgresql.service redis.service
Wants=postgresql.service redis.service

[Service]
Type=exec
User=memesnipe
Group=memesnipe
WorkingDirectory=/opt/memesnipe
ExecStart=/opt/memesnipe/bin/executor
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5
TimeoutStopSec={graceful_shutdown}
KillMode=mixed

# Environment
Environment=DEPLOYMENT_ENV={environment}
Environment=RUST_LOG={log_level}
EnvironmentFile=/opt/memesnipe/.env

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/log/memesnipe /var/lib/memesnipe

# Resource limits
LimitNOFILE=65536
MemoryMax={max_memory}M
CPUQuota={cpu_quota}%

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=memesnipe-executor

[Install]
WantedBy=multi-user.target
"#,
            graceful_shutdown = self.deployment.graceful_shutdown_timeout_seconds,
            environment = self.deployment.environment,
            log_level = self.logging.level,
            max_memory = self.deployment.max_memory_mb,
            cpu_quota = (self.deployment.max_cpu_cores * 100.0) as u32,
        );

        Ok(service)
    }

    pub fn summary(&self) -> String {
        format!(
            "=== MemeSnipe v25 Production Configuration ===\n\
            🚀 Deployment: {} v{} ({} replicas)\n\
            💾 Database: {}:{}/{} (SSL: {})\n\
            📡 Redis: {}:{} (SSL: {})\n\
            🌐 HTTP: {}:{}, WebSocket: {}:{}\n\
            📊 Monitoring: Prometheus({}), Health({})\n\
            📝 Logging: {} level, {} format\n\
            🔐 Security: TLS enabled, {} CORS origins\n\
            💾 Backup: {} (retention: {} days)\n\
            📧 Alerts: {} recipients configured\n\
            \n\
            Resources: {}MB RAM, {} CPU cores\n\
            Performance: {} workers, {}MB cache\n\
            \n\
            Ready for production deployment! 🎯",
            self.deployment.environment,
            self.deployment.version,
            self.deployment.replicas,
            self.database.host,
            self.database.port,
            self.database.database,
            self.database.ssl_mode,
            self.redis.host,
            self.redis.port,
            if self.redis.ssl_enabled {
                "enabled"
            } else {
                "disabled"
            },
            self.networking.bind_address,
            self.networking.http_port,
            self.networking.bind_address,
            self.networking.websocket_port,
            self.monitoring.prometheus_port,
            self.monitoring.health_check_port,
            self.logging.level,
            self.logging.format,
            self.security.cors_allowed_origins.len(),
            if self.backup.enabled {
                "enabled"
            } else {
                "disabled"
            },
            self.backup.backup_retention_days,
            self.alerts.alert_recipients.len(),
            self.deployment.max_memory_mb,
            self.deployment.max_cpu_cores,
            self.performance.worker_threads,
            self.performance.cache_size_mb,
        )
    }
}
