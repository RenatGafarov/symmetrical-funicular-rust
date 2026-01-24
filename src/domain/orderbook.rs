//! Orderbook data structures.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// PriceLevel represents a single price level in the orderbook.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: Decimal,
    pub quantity: Decimal,
}

/// Orderbook represents the current state of bids and asks for a trading pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Orderbook {
    /// The trading pair in "BASE/QUOTE" format (e.g., "BTC/USDT").
    pub pair: String,
    /// The exchange name this orderbook belongs to.
    pub exchange: String,
    /// Sorted list of bid price levels (highest to lowest).
    pub bids: Vec<PriceLevel>,
    /// Sorted list of ask price levels (lowest to highest).
    pub asks: Vec<PriceLevel>,
    /// Timestamp when this orderbook was captured.
    pub timestamp: SystemTime,
}

impl Orderbook {
    /// Returns the best bid price level, if available.
    pub fn best_bid(&self) -> Option<&PriceLevel> {
        self.bids.first()
    }

    /// Returns the best ask price level, if available.
    pub fn best_ask(&self) -> Option<&PriceLevel> {
        self.asks.first()
    }

    /// Returns the spread between best ask and best bid.
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_ask(), self.best_bid()) {
            (Some(ask), Some(bid)) => Some(ask.price - bid.price),
            _ => None,
        }
    }
}
