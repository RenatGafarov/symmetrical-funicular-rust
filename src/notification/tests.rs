//! Tests for notification formatting functions.

use super::*;
use std::time::Duration;

// ==================== Helper function tests ====================

#[test]
fn test_parse_pair_base_btc_usdt() {
    assert_eq!(parse_pair_base("BTC/USDT"), "BTC");
}

#[test]
fn test_parse_pair_base_eth_btc() {
    assert_eq!(parse_pair_base("ETH/BTC"), "ETH");
}

#[test]
fn test_parse_pair_base_no_slash() {
    assert_eq!(parse_pair_base("BTCUSDT"), "BTCUSDT");
}

#[test]
fn test_parse_pair_base_empty() {
    assert_eq!(parse_pair_base(""), "");
}

#[test]
fn test_format_pair_tag_escapes_underscore() {
    // Underscore must be escaped for Telegram Markdown
    assert_eq!(format_pair_tag("BTC/USDT"), "BTC\\_USDT");
}

#[test]
fn test_format_pair_tag_eth_btc() {
    assert_eq!(format_pair_tag("ETH/BTC"), "ETH\\_BTC");
}

#[test]
fn test_format_pair_tag_no_slash() {
    // No slash means no underscore added
    assert_eq!(format_pair_tag("BTCUSDT"), "BTCUSDT");
}

#[test]
fn test_format_duration_seconds() {
    assert_eq!(format_duration(Duration::from_secs(45)), "45с");
}

#[test]
fn test_format_duration_minutes() {
    assert_eq!(format_duration(Duration::from_secs(125)), "2м 5с");
}

#[test]
fn test_format_duration_hours() {
    assert_eq!(format_duration(Duration::from_secs(3725)), "1ч 2м");
}

#[test]
fn test_format_duration_days() {
    assert_eq!(format_duration(Duration::from_secs(90000)), "1д 1ч");
}

#[test]
fn test_format_duration_zero() {
    assert_eq!(format_duration(Duration::ZERO), "0с");
}

#[test]
fn test_add_thousand_separators_small() {
    assert_eq!(add_thousand_separators(42), "42");
}

#[test]
fn test_add_thousand_separators_thousands() {
    assert_eq!(add_thousand_separators(1234), "1,234");
}

#[test]
fn test_add_thousand_separators_millions() {
    assert_eq!(add_thousand_separators(1234567), "1,234,567");
}

#[test]
fn test_add_thousand_separators_zero() {
    assert_eq!(add_thousand_separators(0), "0");
}

// ==================== Event formatting tests ====================

#[test]
fn test_format_opportunity_contains_pair_tag() {
    let data = OpportunityData {
        pair: "BTC/USDT".to_string(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "Bybit".to_string(),
        buy_price: 42000.0,
        sell_price: 42100.0,
        spread_percent: 0.24,
        potential_profit: 10.0,
        quantity: 0.1,
    };

    let msg = format_opportunity(&data);

    // Check escaped underscore in hashtag
    assert!(msg.contains("#BTC\\_USDT"));
    assert!(msg.contains("Binance"));
    assert!(msg.contains("Bybit"));
    assert!(msg.contains("0.24%"));
}

#[test]
fn test_format_startup_dry_run() {
    let data = StartupData {
        version: "1.0.0".to_string(),
        exchanges: vec!["Binance".to_string(), "Bybit".to_string()],
        pairs: vec!["BTC/USDT".to_string()],
        dry_run: true,
    };

    let msg = format_startup(&data);

    assert!(msg.contains("DRY RUN"));
    assert!(msg.contains("1.0.0"));
    assert!(msg.contains("Binance, Bybit"));
}

#[test]
fn test_format_startup_live() {
    let data = StartupData {
        version: "1.0.0".to_string(),
        exchanges: vec!["Binance".to_string()],
        pairs: vec!["ETH/USDT".to_string()],
        dry_run: false,
    };

    let msg = format_startup(&data);

    assert!(msg.contains("LIVE"));
    assert!(!msg.contains("DRY RUN"));
}

#[test]
fn test_format_shutdown_graceful() {
    let data = ShutdownData {
        reason: "User requested".to_string(),
        uptime: Duration::from_secs(3600),
        graceful: true,
    };

    let msg = format_shutdown(&data);

    assert!(msg.contains("Graceful"));
    assert!(msg.contains("User requested"));
    assert!(msg.contains("1ч 0м"));
}

#[test]
fn test_format_shutdown_forced() {
    let data = ShutdownData {
        reason: "Error".to_string(),
        uptime: Duration::from_secs(60),
        graceful: false,
    };

    let msg = format_shutdown(&data);

    assert!(msg.contains("Forced"));
}

#[test]
fn test_format_execution_success() {
    let data = ExecutionData {
        pair: "BTC/USDT".to_string(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "Bybit".to_string(),
        success: true,
        actual_profit: 15.5,
        execution_time: Duration::from_millis(250),
        error_message: None,
    };

    let msg = format_execution(&data);

    assert!(msg.contains("выполнена"));
    assert!(msg.contains("$15.50"));
}

#[test]
fn test_format_execution_failure() {
    let data = ExecutionData {
        pair: "BTC/USDT".to_string(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "Bybit".to_string(),
        success: false,
        actual_profit: 0.0,
        execution_time: Duration::from_millis(100),
        error_message: Some("Insufficient balance".to_string()),
    };

    let msg = format_execution(&data);

    assert!(msg.contains("не выполнена"));
    assert!(msg.contains("Insufficient balance"));
}

#[test]
fn test_format_error() {
    let data = ErrorData {
        component: "OrderExecutor".to_string(),
        message: "Failed to place order".to_string(),
        error: Some("Connection timeout".to_string()),
    };

    let msg = format_error(&data);

    assert!(msg.contains("OrderExecutor"));
    assert!(msg.contains("Failed to place order"));
    assert!(msg.contains("Connection timeout"));
}

#[test]
fn test_format_overview() {
    let data = OverviewData {
        uptime: Duration::from_secs(7200),
        detection_cycles: 1500,
        opportunities_detected: 25,
        opportunities_executed: 20,
        successful_trades: 18,
        failed_trades: 2,
        total_profit: 150.75,
        dry_run: false,
    };

    let msg = format_overview(&data);

    assert!(msg.contains("LIVE"));
    assert!(msg.contains("2ч 0м"));
    assert!(msg.contains("1,500"));
    assert!(msg.contains("$150.75"));
}

// ==================== Event constructor tests ====================

#[test]
fn test_event_opportunity_constructor() {
    let data = OpportunityData {
        pair: "BTC/USDT".to_string(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "Bybit".to_string(),
        buy_price: 42000.0,
        sell_price: 42100.0,
        spread_percent: 0.24,
        potential_profit: 10.0,
        quantity: 0.1,
    };

    let event = Event::opportunity(data);

    assert_eq!(event.event_type, EventType::Opportunity);
}

#[test]
fn test_event_error_constructor() {
    let data = ErrorData {
        component: "Test".to_string(),
        message: "Error".to_string(),
        error: None,
    };

    let event = Event::error(data);

    assert_eq!(event.event_type, EventType::Error);
}

#[test]
fn test_event_type_display() {
    assert_eq!(EventType::Opportunity.to_string(), "opportunity");
    assert_eq!(EventType::Execution.to_string(), "execution");
    assert_eq!(EventType::Error.to_string(), "error");
    assert_eq!(EventType::Startup.to_string(), "startup");
    assert_eq!(EventType::Shutdown.to_string(), "shutdown");
    assert_eq!(EventType::Overview.to_string(), "overview");
}
