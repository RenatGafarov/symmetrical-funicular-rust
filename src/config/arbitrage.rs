//! Arbitrage detection configuration.

use serde::Deserialize;
use std::time::Duration;

use super::duration;

/// Arbitrage detection settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ArbitrageConfig {
    /// Cross-exchange arbitrage detection (optional).
    pub cross_exchange: Option<CrossExchangeConfig>,
    /// Timeout for each detection cycle (default: 10s).
    #[serde(default, with = "duration")]
    pub detection_timeout: Duration,
}

/// Cross-exchange arbitrage settings.
#[derive(Debug, Clone, Deserialize)]
pub struct CrossExchangeConfig {
    /// Minimum profit percentage to trigger (e.g., "0.003" for 0.3%).
    pub min_profit_threshold: Option<String>,
    /// Minimum quantity to trade (e.g., "0.0001").
    pub min_quantity: Option<String>,
    /// How long an opportunity is considered valid (default: 5s).
    #[serde(default, with = "duration")]
    pub opportunity_ttl: Duration,
}
