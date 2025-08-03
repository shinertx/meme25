use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("Database Error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("Serialization Error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Configuration Error: {0}")]
    Config(String),
    #[error("Redis Error: {0}")]
    Redis(String),
    #[error("Network Error: {0}")]
    Network(String),
    #[error("Strategy Error: {0}")]
    Strategy(String),
    #[error("Metrics Error: {0}")]
    Metrics(String),
}

impl From<prometheus::Error> for ModelError {
    fn from(error: prometheus::Error) -> Self {
        ModelError::Metrics(error.to_string())
    }
}

pub type Result<T, E = ModelError> = std::result::Result<T, E>;
