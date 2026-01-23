//! Application-level configuration.

use serde::Deserialize;

/// Application-level settings.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Application name used in logs and metrics.
    pub name: String,
    /// Environment: "development", "staging", or "production".
    pub env: String,
    /// Logging verbosity: "debug", "info", "warn", "error".
    pub log_level: Option<String>,
}
