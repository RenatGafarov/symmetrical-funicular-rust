use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use reqwest::Method;
use rust_decimal::Decimal;
use serde::Deserialize;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use crate::config::{Config, ExchangeConfig};
use crate::domain::{Fees, Order, OrderSide, Orderbook, Trade};
use crate::exchanges::poloniex::{Client, WebSocketManager};
use crate::exchanges::utils::{pair_to_symbol, parse_order_side, parse_order_status, parse_order_type, parse_price_levels, symbol_to_pair};
use crate::exchanges::{Exchange, ExchangeError, Result};

const EXCHANGE_NAME: &str = "poloniex";

/// Maximum acceptable clock drift between local and server time.
const MAX_CLOCK_DRIFT: Duration = Duration::from_secs(5);

/// Poloniex exchange implementation.
pub struct PoloniexExchange {
    client: Client,
    config: ExchangeConfig,
    fees: Fees,
    orderbook_depth: i32,
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

        // Parse taker fee from config, default to 0
        let taker_fee = exchange_config
            .fee_taker
            .as_ref()
            .and_then(|s| Decimal::from_str(s).ok())
            .unwrap_or_default();

        let fees = Fees::new(taker_fee, taker_fee);

        let orderbook_depth = config
            .orderbook
            .as_ref()
            .and_then(|o| o.max_depth)
            .unwrap_or(DEFAULT_ORDERBOOK_DEPTH);

        Ok(Self {
            client,
            config: exchange_config.clone(),
            fees,
            orderbook_depth,
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

    async fn get_orderbook(&self, pair: &str) -> Result<Orderbook> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        let symbol = pair_to_symbol(pair);
        let endpoint = format!("/markets/{}/orderBook", symbol);

        let depth = self.orderbook_depth;

        let mut params = HashMap::new();
        params.insert("limit".to_string(), depth.to_string());

        let body = self
            .client
            .request(Method::GET, &endpoint, Some(params), false)
            .await
            .map_err(|e| ExchangeError::Api(format!("get orderbook for {}: {}", pair, e)))?;

        let resp: OrderbookResponse = serde_json::from_slice(&body)
            .map_err(|e| ExchangeError::Api(format!("parse orderbook: {}", e)))?;

        Ok(resp.to_orderbook(pair))
    }

    async fn subscribe_orderbook(
        &self,
        pairs: Vec<String>,
    ) -> Result<mpsc::UnboundedReceiver<Orderbook>> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        // Create a WebSocket manager
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

    // TODO: мне не нравится концепция IOC, надо вернуться и покрутить логику ордеров
    // https://docs.poloniex.com/#order-types
    async fn place_order(&self, order: Order) -> Result<Trade> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        let symbol = pair_to_symbol(&order.pair);
        let side = match order.side {
            OrderSide::Buy => "BUY",
            OrderSide::Sell => "SELL",
        };

        let mut params = HashMap::new();
        params.insert("symbol".to_string(), symbol);
        params.insert("side".to_string(), side.to_string());
        params.insert("type".to_string(), "LIMIT".to_string());
        params.insert("price".to_string(), order.price.to_string());
        params.insert("quantity".to_string(), order.quantity.to_string());
        params.insert("timeInForce".to_string(), "IOC".to_string()); // Immediate Or Cancel

        let body = self
            .client
            .request(Method::POST, "/orders", Some(params), true)
            .await
            .map_err(|e| map_client_error(e, &order.pair))?;

        let resp: PlaceOrderResponse = serde_json::from_slice(&body)
            .map_err(|e| ExchangeError::Api(format!("parse order response: {}", e)))?;

        let filled_qty = Decimal::from_str(&resp.filled_quantity).unwrap_or_default();
        let avg_price = Decimal::from_str(&resp.avg_price)
            .ok()
            .filter(|p| !p.is_zero())
            .unwrap_or(order.price);

        Ok(Trade {
            id: resp.id.clone(),
            order_id: resp.id,
            exchange: EXCHANGE_NAME.to_string(),
            pair: order.pair,
            side: order.side,
            price: avg_price,
            quantity: filled_qty,
            fee: Decimal::ZERO,
            fee_currency: String::new(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    async fn cancel_order(&self, _order_id: &str) -> Result<()> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        let endpoint = format!("/orders/{}", _order_id);

        self.client
            .request(Method::DELETE, &endpoint, None, true)
            .await
            .map_err(|e| ExchangeError::Api(format!("cancel order: {}", e)))?;

        Ok(())
    }

    async fn get_order(&self, order_id: &str) -> Result<Order> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        let endpoint = format!("/orders/{}", order_id);
        let body = self
            .client
            .request(Method::GET, &endpoint, None, true)
            .await
            .map_err(|e| ExchangeError::Api(format!("get order: {}", e)))?;

        let order_info: OrderInfo = serde_json::from_slice(&body)
            .map_err(|e| ExchangeError::Api(format!("parse order: {}", e)))?;

        Ok(order_info.to_order())
    }

    async fn get_balances(&self) -> Result<HashMap<String, Decimal>> {
        if !self.is_connected() {
            return Err(ExchangeError::Connection("not connected".to_string()));
        }

        let body = self
            .client
            .request(Method::GET, "/accounts/balances", None, true)
            .await
            .map_err(|e| ExchangeError::Api(format!("get balances: {}", e)))?;

        let accounts: Vec<AccountBalance> = serde_json::from_slice(&body)
            .map_err(|e| ExchangeError::Api(format!("parse balances: {}", e)))?;

        let mut balances = HashMap::new();
        for account in accounts {
            if account.account_type != "SPOT" {
                continue;
            }
            for bal in account.balances {
                let available = Decimal::from_str(&bal.available).unwrap_or_default();
                if available.is_sign_positive() && !available.is_zero() {
                    balances.insert(bal.currency, available);
                }
            }
        }

        debug!(balances = ?balances, "fetched balances");

        Ok(balances)
    }

    fn get_fees(&self, _pair: &str) -> Fees {
        self.fees
    }

    fn name(&self) -> &str {
        EXCHANGE_NAME
    }

    fn supported_pairs(&self) -> Vec<String> {
        self.pairs.clone()
    }
}

/// Default orderbook depth.
const DEFAULT_ORDERBOOK_DEPTH: i32 = 20;

/// Poloniex orderbook response.
#[derive(Debug, Deserialize)]
struct OrderbookResponse {
    time: i64,
    #[allow(dead_code)]
    scale: Option<String>,
    asks: Vec<String>,
    bids: Vec<String>,
    ts: i64,
}

/// Poloniex place order response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlaceOrderResponse {
    id: String,
    #[allow(dead_code)]
    client_order_id: String,
    #[allow(dead_code)]
    state: String,
    filled_quantity: String,
    avg_price: String,
}

impl OrderbookResponse {
    fn to_orderbook(&self, pair: &str) -> Orderbook {
        let bids = parse_price_levels(&self.bids);
        let asks = parse_price_levels(&self.asks);
        let timestamp_ms = if self.ts != 0 { self.ts } else { self.time };

        Orderbook {
            exchange: EXCHANGE_NAME.to_string(),
            pair: pair.to_string(),
            bids,
            asks,
            timestamp: UNIX_EPOCH + Duration::from_millis(timestamp_ms as u64),
        }
    }
}

/// Poloniex account balance response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountBalance {
    #[allow(dead_code)]
    account_id: String,
    account_type: String,
    balances: Vec<Balance>,
}

/// Individual currency balance.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Balance {
    #[allow(dead_code)]
    currency_id: String,
    currency: String,
    available: String,
    #[allow(dead_code)]
    hold: String,
}

/// Poloniex order info response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderInfo {
    id: String,
    #[allow(dead_code)]
    client_order_id: String,
    symbol: String,
    side: String,
    #[serde(rename = "type")]
    order_type: String,
    price: String,
    quantity: String,
    #[allow(dead_code)]
    amount: String,
    state: String,
    #[allow(dead_code)]
    filled_amount: String,
    #[allow(dead_code)]
    filled_quantity: String,
    create_time: i64,
    update_time: i64,
}

impl OrderInfo {
    fn to_order(&self) -> Order {
        let price = Decimal::from_str(&self.price).unwrap_or_default();
        let quantity = Decimal::from_str(&self.quantity).unwrap_or_default();

        Order {
            id: self.id.clone(),
            exchange: EXCHANGE_NAME.to_string(),
            pair: symbol_to_pair(&self.symbol),
            side: parse_order_side(&self.side),
            order_type: parse_order_type(&self.order_type),
            price,
            quantity,
            status: parse_order_status(&self.state),
            created_at: UNIX_EPOCH + Duration::from_millis(self.create_time as u64),
            updated_at: UNIX_EPOCH + Duration::from_millis(self.update_time as u64),
        }
    }
}

/// Maps Poloniex client errors to exchange errors.
fn map_client_error(err: crate::exchanges::poloniex::client::ClientError, pair: &str) -> ExchangeError {
    use crate::exchanges::poloniex::client::ClientError;

    match err {
        ClientError::Api(api_err) => match api_err.code {
            21603 => ExchangeError::InsufficientFunds,
            21606 => ExchangeError::OrderNotFound(pair.to_string()),
            21601 => ExchangeError::PairNotSupported(pair.to_string()),
            _ => ExchangeError::Api(format!("poloniex error for {}: {}", pair, api_err)),
        },
        ClientError::RateLimitExceeded { .. } => {
            ExchangeError::Api(format!("rate limit exceeded for {}", pair))
        }
        other => ExchangeError::Api(format!("{}", other)),
    }
}