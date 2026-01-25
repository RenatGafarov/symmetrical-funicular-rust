//! Manager for handling multiple exchange connections.

use super::{Exchange, ExchangeError, Result};
use crate::config::{Config, ExchangeConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use super::poloniex;

/// Manager coordinates multiple exchange connections.
pub struct Manager {
    /// Map of exchange name to exchange instance.
    exchanges: Arc<RwLock<HashMap<String, Arc<dyn Exchange>>>>,
}

impl Manager {
    /// Creates a new Manager instance.
    pub fn new() -> Self {
        Self {
            exchanges: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a new Manager from configuration.
    /// Only enabled exchanges will be instantiated.
    pub async fn from_config(config: &Config) -> Result<Self> {
        let manager = Self::new();

        for (name, exchange_config) in &config.exchanges {
            if !exchange_config.enabled {
                info!(exchange = %name, "Skipping disabled exchange");
                continue;
            }

            info!(exchange = %name, "Loading exchange from config");

            let exchange = Self::create_exchange(name, exchange_config)?;
            manager.register(exchange).await;
        }

        Ok(manager)
    }

    /// Factory method to create an exchange instance based on name and config.
    fn create_exchange(
        name: &str,
        config: &ExchangeConfig,
    ) -> Result<Arc<dyn Exchange>> {
        match name.to_lowercase().as_str() {
            "poloniex" => {
                // TODO: Implement Poloniex exchange
                Err(ExchangeError::Internal(format!(
                    "exchange {} is not yet implemented",
                    name
                )))
            }
            "gate" | "gateio" | "gate.io" => {
                // TODO: Implement Gate.io exchange
                Err(ExchangeError::Internal(format!(
                    "exchange {} is not yet implemented",
                    name
                )))
            }
            _ => Err(ExchangeError::Internal(format!(
                "unknown exchange: {}",
                name
            ))),
        }
    }

    /// Registers a new exchange with the manager.
    pub async fn register(&self, exchange: Arc<dyn Exchange>) {
        let name = exchange.name().to_string();
        let mut exchanges = self.exchanges.write().await;
        info!(exchange = %name, "Registering exchange");
        exchanges.insert(name, exchange);
    }

    /// Unregisters an exchange by name.
    pub async fn unregister(&self, name: &str) -> Result<()> {
        let mut exchanges = self.exchanges.write().await;
        if exchanges.remove(name).is_some() {
            info!(exchange = %name, "Unregistered exchange");
            Ok(())
        } else {
            warn!(exchange = %name, "Attempted to unregister unknown exchange");
            Err(ExchangeError::Internal(format!(
                "exchange {} not found",
                name
            )))
        }
    }

    /// Returns a reference to an exchange by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Exchange>> {
        let exchanges = self.exchanges.read().await;
        exchanges.get(name).cloned()
    }

    /// Returns all registered exchange names.
    pub async fn list(&self) -> Vec<String> {
        let exchanges = self.exchanges.read().await;
        exchanges.keys().cloned().collect()
    }

    /// Connects all registered exchanges.
    pub async fn connect_all(&self) -> Result<()> {
        let exchanges = self.exchanges.read().await;
        for (name, exchange) in exchanges.iter() {
            info!(exchange = %name, "Connecting to exchange");
            if let Err(e) = exchange.connect().await {
                error!(exchange = %name, error = %e, "Failed to connect to exchange");
                return Err(e);
            }
        }
        Ok(())
    }

    /// Disconnects all registered exchanges.
    pub async fn disconnect_all(&self) -> Result<()> {
        let exchanges = self.exchanges.read().await;
        for (name, exchange) in exchanges.iter() {
            info!(exchange = %name, "Disconnecting from exchange");
            if let Err(e) = exchange.disconnect().await {
                error!(exchange = %name, error = %e, "Failed to disconnect from exchange");
                // Continue disconnecting other exchanges even if one fails
            }
        }
        Ok(())
    }

    /// Returns connection status for all exchanges.
    pub async fn status(&self) -> HashMap<String, bool> {
        let exchanges = self.exchanges.read().await;
        exchanges
            .iter()
            .map(|(name, exchange)| (name.clone(), exchange.is_connected()))
            .collect()
    }
}

impl Default for Manager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Fees, Order, Orderbook, Trade};
    use async_trait::async_trait;
    use rust_decimal::Decimal;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::sync::mpsc;

    /// Mock exchange for testing.
    struct MockExchange {
        name: String,
        connected: AtomicBool,
        should_fail_connect: bool,
        supported_pairs: Vec<String>,
        fees: Fees,
    }

    impl MockExchange {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                connected: AtomicBool::new(false),
                should_fail_connect: false,
                supported_pairs: vec!["BTC/USDT".to_string(), "ETH/USDT".to_string()],
                fees: Fees {
                    maker: Decimal::new(1, 3), // 0.001 = 0.1%
                    taker: Decimal::new(2, 3), // 0.002 = 0.2%
                },
            }
        }

        fn with_fail_connect(mut self) -> Self {
            self.should_fail_connect = true;
            self
        }
    }

    #[async_trait]
    impl Exchange for MockExchange {
        async fn connect(&self) -> Result<()> {
            if self.should_fail_connect {
                return Err(ExchangeError::Connection("mock connection failure".into()));
            }
            self.connected.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn disconnect(&self) -> Result<()> {
            self.connected.store(false, Ordering::SeqCst);
            Ok(())
        }

        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }

        async fn get_orderbook(&self, _pair: &str) -> Result<Orderbook> {
            unimplemented!("not needed for manager tests")
        }

        async fn subscribe_orderbook(
            &self,
            _pairs: Vec<String>,
        ) -> Result<mpsc::UnboundedReceiver<Orderbook>> {
            unimplemented!("not needed for manager tests")
        }

        async fn place_order(&self, _order: Order) -> Result<Trade> {
            unimplemented!("not needed for manager tests")
        }

        async fn cancel_order(&self, _order_id: &str) -> Result<()> {
            unimplemented!("not needed for manager tests")
        }

        async fn get_order(&self, _order_id: &str) -> Result<Order> {
            unimplemented!("not needed for manager tests")
        }

        async fn get_balances(&self) -> Result<HashMap<String, Decimal>> {
            unimplemented!("not needed for manager tests")
        }

        fn get_fees(&self, _pair: &str) -> Fees {
            self.fees.clone()
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn supported_pairs(&self) -> Vec<String> {
            self.supported_pairs.clone()
        }
    }

    #[tokio::test]
    async fn test_new_manager_is_empty() {
        let manager = Manager::new();
        let exchanges = manager.list().await;
        assert!(exchanges.is_empty());
    }

    #[tokio::test]
    async fn test_register_exchange() {
        let manager = Manager::new();
        let exchange = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;

        manager.register(exchange).await;

        let exchanges = manager.list().await;
        assert_eq!(exchanges.len(), 1);
        assert!(exchanges.contains(&"binance".to_string()));
    }

    #[tokio::test]
    async fn test_register_multiple_exchanges() {
        let manager = Manager::new();
        let binance = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        let bybit = Arc::new(MockExchange::new("bybit")) as Arc<dyn Exchange>;

        manager.register(binance).await;
        manager.register(bybit).await;

        let mut exchanges = manager.list().await;
        exchanges.sort();
        assert_eq!(exchanges.len(), 2);
        assert_eq!(exchanges, vec!["binance", "bybit"]);
    }

    #[tokio::test]
    async fn test_get_existing_exchange() {
        let manager = Manager::new();
        let exchange = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        manager.register(exchange).await;

        let result = manager.get("binance").await;
        assert!(result.is_some());
        assert_eq!(result.unwrap().name(), "binance");
    }

    #[tokio::test]
    async fn test_get_nonexistent_exchange() {
        let manager = Manager::new();
        let result = manager.get("binance").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_unregister_existing_exchange() {
        let manager = Manager::new();
        let exchange = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        manager.register(exchange).await;

        let result = manager.unregister("binance").await;
        assert!(result.is_ok());

        let exchanges = manager.list().await;
        assert!(exchanges.is_empty());
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_exchange() {
        let manager = Manager::new();
        let result = manager.unregister("binance").await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ExchangeError::Internal(_))));
    }

    #[tokio::test]
    async fn test_connect_all_success() {
        let manager = Manager::new();
        let binance = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        let bybit = Arc::new(MockExchange::new("bybit")) as Arc<dyn Exchange>;

        manager.register(binance.clone()).await;
        manager.register(bybit.clone()).await;

        let result = manager.connect_all().await;
        assert!(result.is_ok());

        let status = manager.status().await;
        assert_eq!(status.get("binance"), Some(&true));
        assert_eq!(status.get("bybit"), Some(&true));
    }

    #[tokio::test]
    async fn test_connect_all_with_failure() {
        let manager = Manager::new();
        let binance = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        let failing = Arc::new(MockExchange::new("failing").with_fail_connect()) as Arc<dyn Exchange>;

        manager.register(binance).await;
        manager.register(failing).await;

        let result = manager.connect_all().await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ExchangeError::Connection(_))));
    }

    #[tokio::test]
    async fn test_disconnect_all() {
        let manager = Manager::new();
        let binance = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        let bybit = Arc::new(MockExchange::new("bybit")) as Arc<dyn Exchange>;

        manager.register(binance).await;
        manager.register(bybit).await;

        // Connect first
        manager.connect_all().await.unwrap();
        assert_eq!(manager.status().await.get("binance"), Some(&true));

        // Then disconnect
        let result = manager.disconnect_all().await;
        assert!(result.is_ok());

        let status = manager.status().await;
        assert_eq!(status.get("binance"), Some(&false));
        assert_eq!(status.get("bybit"), Some(&false));
    }

    #[tokio::test]
    async fn test_status_empty_manager() {
        let manager = Manager::new();
        let status = manager.status().await;
        assert!(status.is_empty());
    }

    #[tokio::test]
    async fn test_status_with_mixed_connections() {
        let manager = Manager::new();
        let binance = Arc::new(MockExchange::new("binance")) as Arc<dyn Exchange>;
        let bybit = Arc::new(MockExchange::new("bybit")) as Arc<dyn Exchange>;

        manager.register(binance.clone()).await;
        manager.register(bybit).await;

        // Connect only binance
        binance.connect().await.unwrap();

        let status = manager.status().await;
        assert_eq!(status.get("binance"), Some(&true));
        assert_eq!(status.get("bybit"), Some(&false));
    }

    #[tokio::test]
    async fn test_default_creates_empty_manager() {
        let manager = Manager::default();
        let exchanges = manager.list().await;
        assert!(exchanges.is_empty());
    }

    #[tokio::test]
    async fn test_from_config_with_no_enabled_exchanges() {
        use crate::config::{AppConfig, Config, ExchangeConfig};

        let config = Config {
            app: AppConfig {
                name: "test".to_string(),
                env: "test".to_string(),
                log_level: None,
            },
            exchanges: HashMap::from([(
                "binance".to_string(),
                ExchangeConfig {
                    enabled: false,
                    testnet: false,
                    api_key: String::new(),
                    api_secret: String::new(),
                    fee_taker: Some("0.001".to_string()),
                    rate_limit: None,
                    websocket: None,
                },
            )]),
            orderbook: None,
            arbitrage: None,
            execution: None,
            risk: None,
            pairs: vec!["BTC/USDT".to_string()],
            notification: None,
            storage: None,
            balance: None,
        };

        let manager = Manager::from_config(&config).await.unwrap();
        let exchanges = manager.list().await;
        assert!(exchanges.is_empty());
    }

    #[tokio::test]
    async fn test_from_config_with_unknown_exchange() {
        use crate::config::{AppConfig, Config, ExchangeConfig};

        let config = Config {
            app: AppConfig {
                name: "test".to_string(),
                env: "test".to_string(),
                log_level: None,
            },
            exchanges: HashMap::from([(
                "unknown_exchange".to_string(),
                ExchangeConfig {
                    enabled: true,
                    testnet: false,
                    api_key: String::new(),
                    api_secret: String::new(),
                    fee_taker: Some("0.001".to_string()),
                    rate_limit: None,
                    websocket: None,
                },
            )]),
            orderbook: None,
            arbitrage: None,
            execution: None,
            risk: None,
            pairs: vec!["BTC/USDT".to_string()],
            notification: None,
            storage: None,
            balance: None,
        };

        let result = Manager::from_config(&config).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(ExchangeError::Internal(_))));
    }
}
