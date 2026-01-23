//! Orderbook configuration.

use serde::Deserialize;
use std::time::Duration;

use super::duration;

/// Orderbook caching settings.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookConfig {
    /// Maximum number of price levels to store per side.
    pub max_depth: Option<i32>,
    /// Maximum age of orderbook data before it's considered stale.
    #[serde(default, with = "duration")]
    pub max_age: Duration,
}
