//! Arbitrage opportunity domain model.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// OpportunityType indicates the type of arbitrage opportunity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpportunityType {
    /// Cross-exchange arbitrage between two different exchanges.
    CrossExchange,
}

impl std::fmt::Display for OpportunityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpportunityType::CrossExchange => write!(f, "cross_exchange"),
        }
    }
}

impl std::str::FromStr for OpportunityType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "cross_exchange" => Ok(OpportunityType::CrossExchange),
            _ => Err(format!("Unknown opportunity type: {}", s)),
        }
    }
}

/// Opportunity represents a detected arbitrage opportunity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Opportunity {
    /// Unique identifier for this opportunity.
    pub id: String,
    /// Type indicates cross-exchange or triangular arbitrage.
    #[serde(rename = "type")]
    pub opportunity_type: OpportunityType,
    /// Trading pair (e.g., "BTC/USDT").
    pub pair: String,
    /// Exchange where to buy (for cross-exchange).
    pub buy_exchange: String,
    /// Exchange where to sell (for cross-exchange).
    pub sell_exchange: String,
    /// Ask price on the buy exchange.
    pub buy_price: Decimal,
    /// Bid price on the sell exchange.
    pub sell_price: Decimal,
    /// Maximum quantity that can be traded.
    pub quantity: Decimal,
    /// Profit before fees.
    pub gross_profit: Decimal,
    /// Profit after all fees.
    pub net_profit: Decimal,
    /// Net profit as a percentage of the trade value.
    pub profit_percent: Decimal,
    /// Taker fee on the buy exchange.
    pub buy_fee: Decimal,
    /// Taker fee on the sell exchange.
    pub sell_fee: Decimal,
    /// When this opportunity was detected.
    pub detected_at: DateTime<Utc>,
    /// When this opportunity is considered stale.
    pub expires_at: DateTime<Utc>,
}

impl Opportunity {
    /// Returns true if the opportunity has expired.
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Returns true if the net profit is positive.
    pub fn is_profitable(&self) -> bool {
        self.net_profit > Decimal::ZERO
    }
}
