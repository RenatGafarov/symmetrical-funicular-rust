//! Execution configuration.

use serde::Deserialize;
use std::time::Duration;

use super::duration;

/// Order execution settings.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    /// Maximum time to wait for order execution.
    #[serde(default, with = "duration")]
    pub timeout: Duration,
    /// Retry behavior for failed orders.
    pub retry: Option<RetryConfig>,
}

/// Retry settings for failed operations.
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_attempts: Option<i32>,
    /// Delay before the first retry.
    #[serde(default, with = "duration")]
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    #[serde(default, with = "duration")]
    pub max_delay: Duration,
    /// Factor by which delay increases after each retry.
    pub multiplier: Option<f64>,
}
