//! Configuration error types.

use thiserror::Error;

/// Configuration loading error.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFile(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("validation failed: {0}")]
    Validation(String),
}
