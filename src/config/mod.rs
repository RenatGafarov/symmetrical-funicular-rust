//! Configuration loading and validation for the arbitrage bot.
//!
//! Uses serde_yaml to load YAML configuration files with support for
//! environment variable overrides for sensitive credentials.

use serde::Deserialize;
use std::{collections::HashMap, env, fs, time::Duration};
use thiserror::Error;

/// Configuration loading error.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFile(#[from] std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("validation failed: {0}")]
    Validation(String),
}

/// Root configuration structure for the arbitrage bot.
///
/// Required sections: app, exchanges, pairs.
/// Optional sections: orderbook, arbitrage, execution, risk, notification, storage, balance.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Application-level settings like name and environment.
    pub app: AppConfig,
    /// Maps exchange names to their configurations.
    pub exchanges: HashMap<String, ExchangeConfig>,
    /// Orderbook caching and staleness settings (optional).
    pub orderbook: Option<OrderbookConfig>,
    /// Arbitrage detection strategies (optional).
    pub arbitrage: Option<ArbitrageConfig>,
    /// Order execution timeouts and retries (optional).
    pub execution: Option<ExecutionConfig>,
    /// Risk management limits (optional).
    pub risk: Option<RiskConfig>,
    /// List of trading pairs to monitor (e.g., "BTC/USDT").
    pub pairs: Vec<String>,
    /// Alert channels like Telegram (optional).
    pub notification: Option<NotificationConfig>,
    /// Opportunity persistence (optional).
    pub storage: Option<StorageConfig>,
    /// Balance caching and sync (optional).
    pub balance: Option<BalanceConfig>,
}

/// Application-level settings.
#[derive(Debug, Deserialize)]
pub struct AppConfig {
    /// Application name used in logs and metrics.
    pub name: String,
    /// Environment: "development", "staging", or "production".
    pub env: String,
    /// Logging verbosity: "debug", "info", "warn", "error".
    pub log_level: Option<String>,
}

/// Settings for a single exchange.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub struct WebSocketConfig {
    /// Whether WebSocket should be used for real-time data.
    #[serde(default)]
    pub enabled: bool,
    /// Interval between ping messages to keep connection alive.
    #[serde(default, with = "duration_serde")]
    pub ping_interval: Duration,
    /// Delay before attempting to reconnect after disconnection.
    #[serde(default, with = "duration_serde")]
    pub reconnect_delay: Duration,
}

/// Orderbook caching settings.
#[derive(Debug, Deserialize)]
pub struct OrderbookConfig {
    /// Maximum number of price levels to store per side.
    pub max_depth: Option<i32>,
    /// Maximum age of orderbook data before it's considered stale.
    #[serde(default, with = "duration_serde")]
    pub max_age: Duration,
}

/// Arbitrage detection settings.
#[derive(Debug, Deserialize)]
pub struct ArbitrageConfig {
    /// Cross-exchange arbitrage detection (optional).
    pub cross_exchange: Option<CrossExchangeConfig>,
    /// Timeout for each detection cycle (default: 10s).
    #[serde(default, with = "duration_serde")]
    pub detection_timeout: Duration,
}

/// Cross-exchange arbitrage settings.
#[derive(Debug, Deserialize)]
pub struct CrossExchangeConfig {
    /// Minimum profit percentage to trigger (e.g., "0.003" for 0.3%).
    pub min_profit_threshold: Option<String>,
    /// Minimum quantity to trade (e.g., "0.0001").
    pub min_quantity: Option<String>,
    /// How long an opportunity is considered valid (default: 5s).
    #[serde(default, with = "duration_serde")]
    pub opportunity_ttl: Duration,
}

/// Order execution settings.
#[derive(Debug, Deserialize)]
pub struct ExecutionConfig {
    /// Maximum time to wait for order execution.
    #[serde(default, with = "duration_serde")]
    pub timeout: Duration,
    /// Retry behavior for failed orders.
    pub retry: Option<RetryConfig>,
}

/// Retry settings for failed operations.
#[derive(Debug, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_attempts: Option<i32>,
    /// Delay before the first retry.
    #[serde(default, with = "duration_serde")]
    pub initial_delay: Duration,
    /// Maximum delay between retries.
    #[serde(default, with = "duration_serde")]
    pub max_delay: Duration,
    /// Factor by which delay increases after each retry.
    pub multiplier: Option<f64>,
}

/// Risk management settings.
#[derive(Debug, Deserialize)]
pub struct RiskConfig {
    /// Maximum position size per exchange as a decimal (e.g., "0.20" for 20%).
    pub max_position_per_exchange: Option<String>,
    /// Maximum daily loss before stopping trading (e.g., "0.05" for 5%).
    pub daily_loss_limit: Option<String>,
    /// Drawdown threshold that triggers emergency stop (e.g., "0.05" for 5%).
    pub kill_switch_drawdown: Option<String>,
    /// Maximum number of open orders allowed.
    pub max_open_orders: Option<i32>,
}

/// Notification settings.
#[derive(Debug, Deserialize)]
pub struct NotificationConfig {
    /// Telegram bot notifications.
    pub telegram: Option<TelegramConfig>,
}

/// Telegram notification settings.
#[derive(Debug, Deserialize)]
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
    #[serde(default, with = "duration_serde")]
    pub overview_interval: Duration,
}

/// Opportunity storage settings.
#[derive(Debug, Deserialize)]
pub struct StorageConfig {
    /// Whether opportunity storage is active.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the SQLite database file.
    pub path: Option<String>,
}

/// Balance caching and sync settings.
#[derive(Debug, Deserialize)]
pub struct BalanceConfig {
    /// Whether balance caching and sync should be used.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Interval for periodic balance sync with exchanges (default: 30s).
    #[serde(default, with = "duration_serde")]
    pub sync_interval: Duration,
    /// Maximum age of balance data before it's considered stale (default: 60s).
    #[serde(default, with = "duration_serde")]
    pub max_age: Duration,
    /// Enable balance sync after each trade execution (default: true).
    #[serde(default = "default_true")]
    pub sync_after_trade: bool,
}

fn default_true() -> bool {
    true
}

/// Custom serde module for parsing duration strings like "30s", "5m", "1h".
mod duration_serde {
    use serde::{self, Deserialize, Deserializer};
    use std::time::Duration;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => parse_duration(&s).map_err(serde::de::Error::custom),
            None => Ok(Duration::ZERO),
        }
    }

    pub(crate) fn parse_duration(s: &str) -> Result<Duration, String> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(Duration::ZERO);
        }

        // Find where the number ends and the unit begins
        let num_end = s
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(s.len());

        let (num_str, unit) = s.split_at(num_end);
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("invalid duration number: {}", num_str))?;

        let multiplier = match unit.trim() {
            "ns" => 1e-9,
            "us" | "µs" => 1e-6,
            "ms" => 1e-3,
            "s" | "" => 1.0,
            "m" => 60.0,
            "h" => 3600.0,
            _ => return Err(format!("unknown duration unit: {}", unit)),
        };

        Ok(Duration::from_secs_f64(num * multiplier))
    }
}

impl Config {
    /// Load configuration from a YAML file at the given path.
    ///
    /// API keys are loaded from environment variables:
    /// - `{EXCHANGE}_API_KEY`, `{EXCHANGE}_API_SECRET`
    /// - `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`, `TELEGRAM_ERROR_CHAT_ID`
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        let mut config: Config = serde_yaml::from_str(&content)?;

        config.load_credentials_from_env();
        config.validate()?;

        Ok(config)
    }

    /// Load credentials from environment variables.
    fn load_credentials_from_env(&mut self) {
        // Load exchange credentials
        for (name, exchange) in self.exchanges.iter_mut() {
            if !exchange.enabled {
                continue;
            }

            let env_prefix = name.to_uppercase();
            exchange.api_key = env::var(format!("{}_API_KEY", env_prefix)).unwrap_or_default();
            exchange.api_secret =
                env::var(format!("{}_API_SECRET", env_prefix)).unwrap_or_default();
        }

        // Load Telegram credentials
        if let Some(ref mut notification) = self.notification {
            if let Some(ref mut telegram) = notification.telegram {
                if telegram.enabled {
                    telegram.bot_token = env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default();
                    telegram.chat_id = env::var("TELEGRAM_CHAT_ID").unwrap_or_default();
                    telegram.error_chat_id = env::var("TELEGRAM_ERROR_CHAT_ID").unwrap_or_default();
                }
            }
        }
    }

    /// Validate the configuration.
    fn validate(&self) -> Result<(), ConfigError> {
        if self.app.name.is_empty() {
            return Err(ConfigError::Validation("app.name is required".into()));
        }

        if self.pairs.is_empty() {
            return Err(ConfigError::Validation(
                "at least one trading pair is required".into(),
            ));
        }

        let mut enabled_exchanges = 0;
        for (name, exchange) in &self.exchanges {
            if exchange.enabled {
                enabled_exchanges += 1;

                if exchange.fee_taker.is_none() {
                    return Err(ConfigError::Validation(format!(
                        "exchange {}: fee_taker is required",
                        name
                    )));
                }

                if exchange.api_key.is_empty() || exchange.api_secret.is_empty() {
                    return Err(ConfigError::Validation(format!(
                        "exchange {}: API credentials not found (set {}_API_KEY and {}_API_SECRET env vars)",
                        name,
                        name.to_uppercase(),
                        name.to_uppercase()
                    )));
                }
            }
        }

        if enabled_exchanges == 0 {
            return Err(ConfigError::Validation(
                "at least one exchange must be enabled".into(),
            ));
        }

        if let Some(ref risk) = self.risk {
            if let Some(max_open_orders) = risk.max_open_orders {
                if max_open_orders <= 0 {
                    return Err(ConfigError::Validation(
                        "risk.max_open_orders must be positive".into(),
                    ));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
