//! Bot error types.

/// Bot error type.
#[derive(Debug, thiserror::Error)]
pub enum BotError {
    #[error("bot is already running")]
    AlreadyRunning,
    #[error("bot is not running")]
    NotRunning,
    #[error("config error: {0}")]
    Config(String),
    #[error("exchange error: {0}")]
    Exchange(String),
    #[error("notification error: {0}")]
    Notification(String),
}
