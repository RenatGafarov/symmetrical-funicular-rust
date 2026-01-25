//! Poloniex exchange integration.

mod exchange;
mod client;
mod websocket;

pub use client::{Client, ClientConfig};
pub use websocket::WebSocketManager;
pub use exchange::PoloniexExchange;