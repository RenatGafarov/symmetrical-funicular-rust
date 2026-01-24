//! Trading fee structures.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Fees represents the trading fees for a pair on an exchange.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fees {
    /// Maker fee (for limit orders that add liquidity).
    /// Expressed as a decimal (e.g., 0.001 for 0.1%).
    pub maker: Decimal,
    /// Taker fee (for orders that remove liquidity).
    /// Expressed as a decimal (e.g., 0.001 for 0.1%).
    pub taker: Decimal,
}

impl Fees {
    /// Creates a new Fees instance.
    pub fn new(maker: Decimal, taker: Decimal) -> Self {
        Self { maker, taker }
    }
}
