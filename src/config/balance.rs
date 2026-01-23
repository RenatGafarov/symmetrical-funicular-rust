//! Balance configuration.

use serde::Deserialize;
use std::time::Duration;

use super::duration;

/// Balance caching and sync settings.
#[derive(Debug, Clone, Deserialize)]
pub struct BalanceConfig {
    /// Whether balance caching and sync should be used.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Interval for periodic balance sync with exchanges (default: 30s).
    #[serde(default, with = "duration")]
    pub sync_interval: Duration,
    /// Maximum age of balance data before it's considered stale (default: 60s).
    #[serde(default, with = "duration")]
    pub max_age: Duration,
    /// Enable balance sync after each trade execution (default: true).
    #[serde(default = "default_true")]
    pub sync_after_trade: bool,
}

fn default_true() -> bool {
    true
}
