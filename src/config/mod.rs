//! Configuration loading and validation for the arbitrage bot.
//!
//! Uses serde_yaml to load YAML configuration files with support for
//! environment variable overrides for sensitive credentials.

mod app;
mod arbitrage;
mod balance;
mod duration;
mod error;
mod exchange;
mod execution;
mod notification;
mod orderbook;
mod risk;
mod storage;

pub use app::AppConfig;
pub use arbitrage::{ArbitrageConfig, CrossExchangeConfig};
pub use balance::BalanceConfig;
pub use error::ConfigError;
pub use exchange::{ExchangeConfig, WebSocketConfig};
pub use execution::{ExecutionConfig, RetryConfig};
pub use notification::{NotificationConfig, TelegramConfig};
pub use orderbook::OrderbookConfig;
pub use risk::RiskConfig;
pub use storage::StorageConfig;

use serde::Deserialize;
use std::{collections::HashMap, env, fs};

/// Root configuration structure for the arbitrage bot.
///
/// Required sections: app, exchanges, pairs.
/// Optional sections: orderbook, arbitrage, execution, risk, notification, storage, balance.
#[derive(Debug, Clone, Deserialize)]
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

impl Config {
    /// Load configuration from a YAML file at the given path.
    ///
    /// First loads environment variables from `.env` file (if exists),
    /// then loads YAML config and credentials from environment variables:
    /// - `{EXCHANGE}_API_KEY`, `{EXCHANGE}_API_SECRET`
    /// - `TELEGRAM_BOT_TOKEN`, `TELEGRAM_CHAT_ID`, `TELEGRAM_ERROR_CHAT_ID`
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        // Load .env file if it exists (ignore error if not found)
        dotenvy::dotenv().ok();

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

        let is_production = self.app.env != "development";

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

                // Only require credentials in production/staging
                if is_production && (exchange.api_key.is_empty() || exchange.api_secret.is_empty())
                {
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
