//! Exchange configuration.

use serde::Deserialize;
use std::time::Duration;

use super::duration;

/// Settings for a single exchange.
#[derive(Debug, Clone, Deserialize)]
pub struct ExchangeConfig {
    /// Whether this exchange should be used.
    #[serde(default)]
    pub enabled: bool,
    /// Enable testnet/sandbox mode.
    #[serde(default)]
    pub testnet: bool,
    /// API key (loaded from environment variable).
    #[serde(skip)]
    pub api_key: String,
    /// API secret (loaded from environment variable).
    #[serde(skip)]
    pub api_secret: String,
    /// Taker fee as a decimal string (e.g., "0.001" for 0.1%).
    pub fee_taker: Option<String>,
    /// Maximum API requests per minute.
    pub rate_limit: Option<i32>,
    /// WebSocket connection settings.
    pub websocket: Option<WebSocketConfig>,
}

/// WebSocket connection settings.
#[derive(Debug, Clone, Deserialize)]
pub struct WebSocketConfig {
    /// Whether WebSocket should be used for real-time data.
    #[serde(default)]
    pub enabled: bool,
    /// Interval between ping messages to keep connection alive.
    #[serde(default, with = "duration")]
    pub ping_interval: Duration,
    /// Delay before attempting to reconnect after disconnection.
    #[serde(default, with = "duration")]
    pub reconnect_delay: Duration,
}
