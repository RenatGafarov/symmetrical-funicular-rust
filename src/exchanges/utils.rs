//! Common utilities for exchange implementations.

use std::str::FromStr;

use rust_decimal::Decimal;

use crate::domain::{OrderSide, OrderStatus, OrderType, PriceLevel};

/// Converts "BTC/USDT" to "BTC_USDT".
pub fn pair_to_symbol(pair: &str) -> String {
    pair.replace('/', "_")
}

/// Converts "BTC_USDT" to "BTC/USDT".
pub fn symbol_to_pair(symbol: &str) -> String {
    symbol.replace('_', "/")
}

/// Parses order side from string.
pub fn parse_order_side(side: &str) -> OrderSide {
    match side.to_uppercase().as_str() {
        "BUY" => OrderSide::Buy,
        _ => OrderSide::Sell,
    }
}

/// Parses order type from string.
pub fn parse_order_type(order_type: &str) -> OrderType {
    match order_type.to_uppercase().as_str() {
        "MARKET" => OrderType::Market,
        _ => OrderType::Limit,
    }
}

/// Maps common order states to OrderStatus.
pub fn parse_order_status(state: &str) -> OrderStatus {
    match state {
        "NEW" | "PARTIALLY_FILLED" => OrderStatus::Open,
        "FILLED" => OrderStatus::Filled,
        "CANCELED" | "PARTIALLY_CANCELED" => OrderStatus::Cancelled,
        "FAILED" | "EXPIRED" => OrderStatus::Failed,
        _ => OrderStatus::Pending,
    }
}

/// Parses a flat array of [price, qty, price, qty, ...] into PriceLevels.
pub fn parse_price_levels(data: &[String]) -> Vec<PriceLevel> {
    data.chunks(2)
        .filter_map(|chunk| {
            if chunk.len() == 2 {
                let price = Decimal::from_str(&chunk[0]).ok()?;
                let quantity = Decimal::from_str(&chunk[1]).ok()?;
                Some(PriceLevel { price, quantity })
            } else {
                None
            }
        })
        .collect()
}