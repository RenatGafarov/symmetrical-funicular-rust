//! Bot configuration.

use crate::config::Config;

/// Bot configuration options.
pub struct BotConfig {
    /// Application configuration.
    pub app_config: Config,
    /// Enable paper trading mode.
    pub dry_run: bool,
    /// Application version.
    pub version: String,
    /// Build timestamp.
    pub build_time: String,
}
