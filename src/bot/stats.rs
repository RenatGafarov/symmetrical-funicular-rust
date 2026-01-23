//! Runtime statistics for the bot.

/// Runtime statistics for the bot.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub detection_cycles: u64,
    pub opportunities_detected: u64,
    pub opportunities_executed: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_profit: f64,
    pub total_volume: f64,
    pub best_trade: f64,
    pub worst_trade: f64,
}
