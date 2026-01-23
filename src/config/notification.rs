//! Notification configuration.

use serde::Deserialize;
use std::time::Duration;

use super::duration;

/// Notification settings.
#[derive(Debug, Clone, Deserialize)]
pub struct NotificationConfig {
    /// Telegram bot notifications.
    pub telegram: Option<TelegramConfig>,
}

/// Telegram notification settings.
#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    /// Whether Telegram notifications are active.
    #[serde(default)]
    pub enabled: bool,
    /// Bot token (loaded from TELEGRAM_BOT_TOKEN env var).
    #[serde(skip)]
    pub bot_token: String,
    /// Target chat/channel ID (loaded from TELEGRAM_CHAT_ID env var).
    #[serde(skip)]
    pub chat_id: String,
    /// Target chat ID for error notifications (loaded from TELEGRAM_ERROR_CHAT_ID env var).
    #[serde(skip)]
    pub error_chat_id: String,
    /// Send alerts when arbitrage opportunities are detected.
    #[serde(default)]
    pub notify_opportunities: bool,
    /// Send alerts when trades are executed.
    #[serde(default)]
    pub notify_executions: bool,
    /// Send alerts when errors occur.
    #[serde(default)]
    pub notify_errors: bool,
    /// Send periodic overview notifications with stats.
    #[serde(default)]
    pub notify_overview: bool,
    /// Interval between overview notifications (default: 1h).
    #[serde(default, with = "duration")]
    pub overview_interval: Duration,
}
