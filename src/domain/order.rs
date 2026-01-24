//! Core business entities for trading orders and trades.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// OrderSide represents the direction of an order (buy or sell).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    /// OrderSideBuy indicates a buy order.
    Buy,
    /// OrderSideSell indicates a sell order.
    Sell,
}

/// OrderType represents the type of order execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderType {
    /// OrderTypeLimit is a limit order that executes at the specified price or better.
    Limit,
    /// OrderTypeMarket is a market order that executes immediately at the best available price.
    Market,
}

/// OrderStatus represents the current state of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    /// OrderStatusPending indicates the order is created but not yet submitted.
    Pending,
    /// OrderStatusOpen indicates the order is submitted and waiting to be filled.
    Open,
    /// OrderStatusFilled indicates the order has been completely filled.
    Filled,
    /// OrderStatusCancelled indicates the order was cancelled before being filled.
    Cancelled,
    /// OrderStatusFailed indicates the order failed due to an error.
    Failed,
}

/// Order represents a trading order on an exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// ID is the unique identifier assigned by the exchange.
    pub id: String,
    /// Exchange is the name of the exchange where the order is placed.
    pub exchange: String,
    /// Pair is the trading pair in "BASE/QUOTE" format (e.g., "BTC/USDT").
    pub pair: String,
    /// Side indicates whether this is a buy or sell order.
    pub side: OrderSide,
    /// Type indicates limit or market order.
    #[serde(rename = "type")]
    pub order_type: OrderType,
    /// Price is the limit price for limit orders (ignored for market orders).
    pub price: Decimal,
    /// Quantity is the amount of base currency to buy or sell.
    pub quantity: Decimal,
    /// Status is the current state of the order.
    pub status: OrderStatus,
    /// CreatedAt is when the order was created.
    pub created_at: SystemTime,
    /// UpdatedAt is when the order was last updated.
    pub updated_at: SystemTime,
}

/// Trade represents an executed trade resulting from an order fill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// ID is the unique identifier for this trade.
    pub id: String,
    /// OrderID is the ID of the order that created this trade.
    pub order_id: String,
    /// Exchange is the name of the exchange where the trade occurred.
    pub exchange: String,
    /// Pair is the trading pair in "BASE/QUOTE" format.
    pub pair: String,
    /// Side indicates buy or sell.
    pub side: OrderSide,
    /// Price is the execution price.
    pub price: Decimal,
    /// Quantity is the amount of base currency traded.
    pub quantity: Decimal,
    /// Fee is the trading fee charged.
    pub fee: Decimal,
    /// FeeCurrency is the currency in which the fee was charged.
    pub fee_currency: String,
    /// Timestamp is when the trade was executed.
    pub timestamp: SystemTime,
}
