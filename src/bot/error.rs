//! Bot error types.

use crate::config::ConfigError;

/// Bot error type.
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("bot is already running")]
    AlreadyRunning,
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}
