//! Exchange integration abstractions and implementations.

mod manager;

use crate::domain::{Fees, Order, Orderbook, Trade};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashMap;
use thiserror::Error;
use tokio::sync::mpsc;

pub use manager::Manager;

/// Exchange errors.
#[derive(Debug, Error)]
pub enum ExchangeError {
    /// Trading pair is not supported by this exchange.
    #[error("pair {0} is not supported")]
    PairNotSupported(String),

    /// Insufficient funds for the operation.
    #[error("insufficient funds")]
    InsufficientFunds,

    /// Order not found.
    #[error("order {0} not found")]
    OrderNotFound(String),

    /// Connection error.
    #[error("connection error: {0}")]
    Connection(String),

    /// API error from the exchange.
    #[error("API error: {0}")]
    Api(String),

    /// Internal error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Result type for exchange operations.
pub type Result<T> = std::result::Result<T, ExchangeError>;

/// Exchange trait defines the interface for cryptocurrency exchange integrations.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Connect establishes connection to the exchange API.
    /// It should initialize WebSocket connections and authenticate if required.
    /// Returns error if connection fails or context is cancelled.
    async fn connect(&self) -> Result<()>;

    /// Disconnect closes all connections to the exchange.
    /// It should gracefully shutdown WebSocket connections and cleanup resources.
    /// Safe to call multiple times.
    async fn disconnect(&self) -> Result<()>;

    /// IsConnected returns true if the exchange connection is active and healthy.
    fn is_connected(&self) -> bool;

    /// GetOrderbook fetches the current orderbook for a trading pair.
    /// The pair format should be "BASE/QUOTE" (e.g., "BTC/USDT").
    /// Returns ErrPairNotSupported if the pair is not available on this exchange.
    async fn get_orderbook(&self, pair: &str) -> Result<Orderbook>;

    /// SubscribeOrderbook opens a real-time orderbook stream for the given pairs.
    /// Returns a channel that receives orderbook updates.
    /// The channel is closed when context is cancelled or connection is lost.
    /// Caller should handle reconnection by calling this method again.
    async fn subscribe_orderbook(
        &self,
        pairs: Vec<String>,
    ) -> Result<mpsc::UnboundedReceiver<Orderbook>>;

    /// PlaceOrder submits a new order to the exchange.
    /// Returns the resulting trade if the order is filled immediately (market orders),
    /// or a trade with zero quantity if the order is placed but not yet filled (limit orders).
    /// Returns ErrInsufficientFunds if balance is not enough.
    async fn place_order(&self, order: Order) -> Result<Trade>;

    /// CancelOrder cancels an open order by its ID.
    /// Returns ErrOrderNotFound if the order doesn't exist or is already filled/cancelled.
    async fn cancel_order(&self, order_id: &str) -> Result<()>;

    /// GetOrder retrieves the current state of an order by its ID.
    /// Returns ErrOrderNotFound if the order doesn't exist.
    async fn get_order(&self, order_id: &str) -> Result<Order>;

    /// GetBalances returns available balances for all assets.
    /// The map keys are asset symbols (e.g., "BTC", "USDT").
    /// Only non-zero balances are included.
    async fn get_balances(&self) -> Result<HashMap<String, Decimal>>;

    /// GetFees returns the maker and taker fees for a trading pair.
    /// Fees are expressed as decimals (e.g., 0.001 for 0.1%).
    fn get_fees(&self, pair: &str) -> Fees;

    /// Name returns the unique identifier of this exchange (e.g., "binance", "bybit").
    fn name(&self) -> &str;

    /// SupportedPairs returns a list of trading pairs available on this exchange.
    /// Pairs are in "BASE/QUOTE" format.
    fn supported_pairs(&self) -> Vec<String>;
}