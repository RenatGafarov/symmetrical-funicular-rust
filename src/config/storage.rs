//! Storage configuration.

use serde::Deserialize;

/// Opportunity storage settings.
#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    /// Whether opportunity storage is active.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the SQLite database file.
    pub path: Option<String>,
}
