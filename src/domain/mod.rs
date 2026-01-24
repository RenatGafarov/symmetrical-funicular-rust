//! Domain models for arbitrage opportunities.

mod fees;
mod opportunity;
mod order;
mod orderbook;

pub use fees::Fees;
pub use opportunity::{Opportunity, OpportunityType};
pub use order::{Order, OrderSide, OrderStatus, OrderType, Trade};
pub use orderbook::{Orderbook, PriceLevel};
