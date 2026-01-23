//! Risk management configuration.

use serde::Deserialize;

/// Risk management settings.
#[derive(Debug, Clone, Deserialize)]
pub struct RiskConfig {
    /// Maximum position size per exchange as a decimal (e.g., "0.20" for 20%).
    pub max_position_per_exchange: Option<String>,
    /// Maximum daily loss before stopping trading (e.g., "0.05" for 5%).
    pub daily_loss_limit: Option<String>,
    /// Drawdown threshold that triggers emergency stop (e.g., "0.05" for 5%).
    pub kill_switch_drawdown: Option<String>,
    /// Maximum number of open orders allowed.
    pub max_open_orders: Option<i32>,
}
