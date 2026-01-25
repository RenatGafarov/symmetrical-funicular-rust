use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rust_decimal::Decimal;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use crate::config::{Config, ExchangeConfig};
use crate::domain::{Fees, Order, Orderbook, Trade};
use crate::exchanges::poloniex::{Client, WebSocketManager};
use crate::exchanges::{Exchange, ExchangeError, Result};

const EXCHANGE_NAME: &str = "poloniex";

/// Maximum acceptable clock drift between local and server time.
const MAX_CLOCK_DRIFT: Duration = Duration::from_secs(5);

/// Poloniex exchange implementation.
pub struct PoloniexExchange {
    client: Client,
    config: ExchangeConfig,
    pairs: Vec<String>,
    connected: AtomicBool,
    websocket_manager: Mutex<Option<Arc<WebSocketManager>>>,
}

impl PoloniexExchange {
    /// Creates a new PoloniexExchange from the application config.
    ///
    /// Returns an error if Poloniex is not configured or not enabled.
    pub fn from_config(config: &Config) -> Result<Self> {
        let exchange_config = config
            .exchanges
            .get(EXCHANGE_NAME)
            .ok_or_else(|| ExchangeError::Internal(format!("{} not found in config", EXCHANGE_NAME)))?;

        if !exchange_config.enabled {
            return Err(ExchangeError::Internal(format!("{} is not enabled", EXCHANGE_NAME)));
        }

        let client = Client::from_config(exchange_config);

        let pairs = config.pairs.clone();

        Ok(Self {
            client,
            config: exchange_config.clone(),
            pairs,
            connected: AtomicBool::new(false),
            websocket_manager: Mutex::new(None),
        })
    }
}

#[async_trait]
impl Exchange for PoloniexExchange {
    async fn connect(&self) -> Result<()> {
        // Check API connectivity by fetching server time
        let server_time = self
            .client
            .get_server_time()
            .await
            .map_err(|e| ExchangeError::Connection(format!("connect to poloniex: {}", e)))?;

        let local_time = chrono::Utc::now();
        let drift = (local_time - server_time).abs();

        info!(
            server_time = %server_time,
            clock_drift = ?drift,
            "connected to poloniex"
        );

        if drift > chrono::Duration::from_std(MAX_CLOCK_DRIFT).unwrap_or_default() {
            warn!(drift = ?drift, "significant clock drift detected");
        }

        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.connected.store(false, Ordering::SeqCst);

        // Close WebSocket manager if it exists
        let guard = self.websocket_manager.lock().await;
        if let Some(ref manager) = *guard {
            manager.close().await;
        }

        debug!("disconnected from {}", EXCHANGE_NAME);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn get_orderbook(&self, _pair: &str) -> Result<Orderbook> {
        todo!()
    }

    async fn subscribe_orderbook(
        &self,
        pairs: Vec<String>,
    ) -> Result<mpsc::UnboundedReceiver<Orderbook>> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        // Create WebSocket manager
        let (manager, orderbook_rx) = WebSocketManager::new(&self.config, pairs);
        let manager = Arc::new(manager);

        // Store manager for later cleanup
        {
            let mut guard = self.websocket_manager.lock().await;
            *guard = Some(Arc::clone(&manager));
        }

        // Spawn subscribe task
        let manager_clone = Arc::clone(&manager);
        tokio::spawn(async move {
            if let Err(e) = manager_clone.subscribe().await {
                warn!(error = %e, "websocket subscription error");
            }
        });

        Ok(orderbook_rx)
    }

    async fn place_order(&self, _order: Order) -> Result<Trade> {
        todo!()
    }

    async fn cancel_order(&self, _order_id: &str) -> Result<()> {
        todo!()
    }

    async fn get_order(&self, _order_id: &str) -> Result<Order> {
        todo!()
    }

    async fn get_balances(&self) -> Result<HashMap<String, Decimal>> {
        todo!()
    }

    fn get_fees(&self, _pair: &str) -> Fees {
        todo!()
    }

    fn name(&self) -> &str {
        "poloniex"
    }

    fn supported_pairs(&self) -> Vec<String> {
        todo!()
    }
}