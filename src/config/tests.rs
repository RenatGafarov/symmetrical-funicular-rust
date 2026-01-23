//! Tests for config module.

use super::*;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

// ==================== Duration parsing tests ====================

#[test]
fn test_parse_duration_seconds() {
    let d = duration::parse_duration("30s").unwrap();
    assert_eq!(d, Duration::from_secs(30));
}

#[test]
fn test_parse_duration_minutes() {
    let d = duration::parse_duration("5m").unwrap();
    assert_eq!(d, Duration::from_secs(300));
}

#[test]
fn test_parse_duration_hours() {
    let d = duration::parse_duration("2h").unwrap();
    assert_eq!(d, Duration::from_secs(7200));
}

#[test]
fn test_parse_duration_milliseconds() {
    let d = duration::parse_duration("100ms").unwrap();
    assert_eq!(d, Duration::from_millis(100));
}

#[test]
fn test_parse_duration_empty() {
    let d = duration::parse_duration("").unwrap();
    assert_eq!(d, Duration::ZERO);
}

#[test]
fn test_parse_duration_invalid_unit() {
    let result = duration::parse_duration("10x");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("unknown duration unit"));
}

#[test]
fn test_parse_duration_fractional() {
    let d = duration::parse_duration("1.5s").unwrap();
    assert_eq!(d, Duration::from_millis(1500));
}

// ==================== YAML field loading tests ====================

/// Parse config from YAML string (for testing).
fn from_yaml(yaml: &str) -> Result<Config, ConfigError> {
    let config: Config = serde_yaml::from_str(yaml)?;
    Ok(config)
}

fn minimal_valid_yaml() -> String {
    r#"
app:
  name: testbot
  env: development

exchanges:
  testex:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#
    .to_string()
}

#[test]
fn test_load_app_fields() {
    let yaml = r#"
app:
  name: mybot
  env: production
  log_level: debug

exchanges:
  binance:
    enabled: false

pairs:
  - ETH/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    assert_eq!(cfg.app.name, "mybot");
    assert_eq!(cfg.app.env, "production");
    assert_eq!(cfg.app.log_level, Some("debug".to_string()));
}

#[test]
fn test_load_exchange_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  binance:
    enabled: true
    testnet: true
    fee_taker: "0.0010"
    rate_limit: 1200
    websocket:
      enabled: true
      ping_interval: 20s
      reconnect_delay: 5s

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let binance = cfg.exchanges.get("binance").unwrap();
    assert!(binance.enabled);
    assert!(binance.testnet);
    assert_eq!(binance.fee_taker, Some("0.0010".to_string()));
    assert_eq!(binance.rate_limit, Some(1200));

    let ws = binance.websocket.as_ref().unwrap();
    assert!(ws.enabled);
    assert_eq!(ws.ping_interval, Duration::from_secs(20));
    assert_eq!(ws.reconnect_delay, Duration::from_secs(5));
}

#[test]
fn test_load_orderbook_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

orderbook:
  max_depth: 20
  max_age: 2s

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let ob = cfg.orderbook.unwrap();
    assert_eq!(ob.max_depth, Some(20));
    assert_eq!(ob.max_age, Duration::from_secs(2));
}

#[test]
fn test_load_arbitrage_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

arbitrage:
  detection_timeout: 10s
  cross_exchange:
    min_profit_threshold: "0.005"
    min_quantity: "0.001"
    opportunity_ttl: 5m

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let arb = cfg.arbitrage.unwrap();
    assert_eq!(arb.detection_timeout, Duration::from_secs(10));

    let ce = arb.cross_exchange.unwrap();
    assert_eq!(ce.min_profit_threshold, Some("0.005".to_string()));
    assert_eq!(ce.min_quantity, Some("0.001".to_string()));
    assert_eq!(ce.opportunity_ttl, Duration::from_secs(300));
}

#[test]
fn test_load_execution_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

execution:
  timeout: 5s
  retry:
    max_attempts: 3
    initial_delay: 100ms
    max_delay: 1s
    multiplier: 2.0

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let exec = cfg.execution.unwrap();
    assert_eq!(exec.timeout, Duration::from_secs(5));

    let retry = exec.retry.unwrap();
    assert_eq!(retry.max_attempts, Some(3));
    assert_eq!(retry.initial_delay, Duration::from_millis(100));
    assert_eq!(retry.max_delay, Duration::from_secs(1));
    assert_eq!(retry.multiplier, Some(2.0));
}

#[test]
fn test_load_risk_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

risk:
  max_position_per_exchange: "0.20"
  daily_loss_limit: "0.05"
  kill_switch_drawdown: "0.03"
  max_open_orders: 10

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let risk = cfg.risk.unwrap();
    assert_eq!(risk.max_position_per_exchange, Some("0.20".to_string()));
    assert_eq!(risk.daily_loss_limit, Some("0.05".to_string()));
    assert_eq!(risk.kill_switch_drawdown, Some("0.03".to_string()));
    assert_eq!(risk.max_open_orders, Some(10));
}

#[test]
fn test_load_notification_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

notification:
  telegram:
    enabled: true
    notify_opportunities: true
    notify_executions: true
    notify_errors: false
    notify_overview: true
    overview_interval: 1h

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let notif = cfg.notification.unwrap();
    let tg = notif.telegram.unwrap();
    assert!(tg.enabled);
    assert!(tg.notify_opportunities);
    assert!(tg.notify_executions);
    assert!(!tg.notify_errors);
    assert!(tg.notify_overview);
    assert_eq!(tg.overview_interval, Duration::from_secs(3600));
}

#[test]
fn test_load_storage_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

storage:
  enabled: true
  path: "data.db"

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let storage = cfg.storage.unwrap();
    assert!(storage.enabled);
    assert_eq!(storage.path, Some("data.db".to_string()));
}

#[test]
fn test_load_balance_fields() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

balance:
  enabled: true
  sync_interval: 30s
  max_age: 60s
  sync_after_trade: true

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let balance = cfg.balance.unwrap();
    assert!(balance.enabled);
    assert_eq!(balance.sync_interval, Duration::from_secs(30));
    assert_eq!(balance.max_age, Duration::from_secs(60));
    assert!(balance.sync_after_trade);
}

#[test]
fn test_load_pairs() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

pairs:
  - BTC/USDT
  - ETH/USDT
  - SOL/USDC
"#;
    let cfg = from_yaml(yaml).unwrap();

    assert_eq!(cfg.pairs.len(), 3);
    assert_eq!(cfg.pairs[0], "BTC/USDT");
    assert_eq!(cfg.pairs[1], "ETH/USDT");
    assert_eq!(cfg.pairs[2], "SOL/USDC");
}

// ==================== Credentials loading tests ====================

#[test]
fn test_load_credentials_from_env() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  testexchange:
    enabled: true
    fee_taker: "0.001"

notification:
  telegram:
    enabled: true

pairs:
  - BTC/USDT
"#;
    let mut cfg = from_yaml(yaml).unwrap();

    // Set env vars (unsafe because modifying env is not thread-safe)
    unsafe {
        env::set_var("TESTEXCHANGE_API_KEY", "test_key_123");
        env::set_var("TESTEXCHANGE_API_SECRET", "test_secret_456");
        env::set_var("TELEGRAM_BOT_TOKEN", "bot_token_789");
        env::set_var("TELEGRAM_CHAT_ID", "chat_id_012");
        env::set_var("TELEGRAM_ERROR_CHAT_ID", "error_chat_345");
    }

    cfg.load_credentials_from_env();

    // Check exchange credentials
    let ex = cfg.exchanges.get("testexchange").unwrap();
    assert_eq!(ex.api_key, "test_key_123");
    assert_eq!(ex.api_secret, "test_secret_456");

    // Check Telegram credentials
    let tg = cfg.notification.unwrap().telegram.unwrap();
    assert_eq!(tg.bot_token, "bot_token_789");
    assert_eq!(tg.chat_id, "chat_id_012");
    assert_eq!(tg.error_chat_id, "error_chat_345");

    // Cleanup
    unsafe {
        env::remove_var("TESTEXCHANGE_API_KEY");
        env::remove_var("TESTEXCHANGE_API_SECRET");
        env::remove_var("TELEGRAM_BOT_TOKEN");
        env::remove_var("TELEGRAM_CHAT_ID");
        env::remove_var("TELEGRAM_ERROR_CHAT_ID");
    }
}

// ==================== Validation tests ====================

#[test]
fn test_validate_empty_app_name() {
    let yaml = r#"
app:
  name: ""
  env: dev

exchanges:
  ex:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#;
    let mut cfg = from_yaml(yaml).unwrap();
    cfg.exchanges.get_mut("ex").unwrap().api_key = "key".to_string();
    cfg.exchanges.get_mut("ex").unwrap().api_secret = "secret".to_string();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("app.name is required"));
}

#[test]
fn test_validate_empty_pairs() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: true
    fee_taker: "0.001"

pairs: []
"#;
    let mut cfg = from_yaml(yaml).unwrap();
    cfg.exchanges.get_mut("ex").unwrap().api_key = "key".to_string();
    cfg.exchanges.get_mut("ex").unwrap().api_secret = "secret".to_string();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("at least one trading pair is required"));
}

#[test]
fn test_validate_no_enabled_exchanges() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: false

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("at least one exchange must be enabled"));
}

#[test]
fn test_validate_missing_fee_taker() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  binance:
    enabled: true

pairs:
  - BTC/USDT
"#;
    let mut cfg = from_yaml(yaml).unwrap();
    cfg.exchanges.get_mut("binance").unwrap().api_key = "key".to_string();
    cfg.exchanges.get_mut("binance").unwrap().api_secret = "secret".to_string();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("fee_taker is required"));
}

#[test]
fn test_validate_missing_credentials_in_production() {
    let yaml = r#"
app:
  name: test
  env: production

exchanges:
  binance:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("API credentials not found"));
}

#[test]
fn test_validate_negative_max_open_orders() {
    let yaml = r#"
app:
  name: test
  env: dev

exchanges:
  ex:
    enabled: true
    fee_taker: "0.001"

risk:
  max_open_orders: -1

pairs:
  - BTC/USDT
"#;
    let mut cfg = from_yaml(yaml).unwrap();
    cfg.exchanges.get_mut("ex").unwrap().api_key = "key".to_string();
    cfg.exchanges.get_mut("ex").unwrap().api_secret = "secret".to_string();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("max_open_orders must be positive"));
}

// ==================== File loading tests ====================

#[test]
fn test_load_from_file_development() {
    // In development mode, credentials are not required
    let yaml = minimal_valid_yaml();

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(yaml.as_bytes()).unwrap();

    // No env vars needed in development mode
    let cfg = Config::load(file.path().to_str().unwrap()).unwrap();

    assert_eq!(cfg.app.name, "testbot");
    assert_eq!(cfg.app.env, "development");
    assert_eq!(cfg.pairs, vec!["BTC/USDT"]);

    let ex = cfg.exchanges.get("testex").unwrap();
    assert!(ex.enabled);
    // Credentials are empty but that's OK in development
    assert!(ex.api_key.is_empty());
}

#[test]
fn test_load_from_file_production_with_credentials() {
    // Use unique exchange name to avoid env var conflicts with parallel tests
    let yaml = r#"
app:
  name: testbot
  env: production

exchanges:
  prodex:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(yaml.as_bytes()).unwrap();

    // Set required env vars for production
    unsafe {
        env::set_var("PRODEX_API_KEY", "prod_key");
        env::set_var("PRODEX_API_SECRET", "prod_secret");
    }

    let cfg = Config::load(file.path().to_str().unwrap()).unwrap();

    assert_eq!(cfg.app.env, "production");
    let ex = cfg.exchanges.get("prodex").unwrap();
    assert_eq!(ex.api_key, "prod_key");
    assert_eq!(ex.api_secret, "prod_secret");

    // Cleanup
    unsafe {
        env::remove_var("PRODEX_API_KEY");
        env::remove_var("PRODEX_API_SECRET");
    }
}

#[test]
fn test_load_file_not_found() {
    let result = Config::load("nonexistent_config.yaml");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("failed to read config file"));
}

// ==================== Environment-specific validation tests ====================

#[test]
fn test_validate_skip_credentials_in_development() {
    // In development mode, credentials are NOT required
    let yaml = r#"
app:
  name: test
  env: development

exchanges:
  binance:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    // Should pass without credentials in development
    let result = cfg.validate();
    assert!(result.is_ok(), "Expected validation to pass in development mode without credentials");
}

#[test]
fn test_validate_require_credentials_in_staging() {
    // In staging mode, credentials ARE required (same as production)
    let yaml = r#"
app:
  name: test
  env: staging

exchanges:
  binance:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#;
    let cfg = from_yaml(yaml).unwrap();

    let result = cfg.validate();
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("API credentials not found"));
}

#[test]
fn test_validate_pass_with_credentials_in_production() {
    // In production mode with credentials, validation should pass
    let yaml = r#"
app:
  name: test
  env: production

exchanges:
  binance:
    enabled: true
    fee_taker: "0.001"

pairs:
  - BTC/USDT
"#;
    let mut cfg = from_yaml(yaml).unwrap();
    cfg.exchanges.get_mut("binance").unwrap().api_key = "key".to_string();
    cfg.exchanges.get_mut("binance").unwrap().api_secret = "secret".to_string();

    let result = cfg.validate();
    assert!(result.is_ok(), "Expected validation to pass in production with credentials");
}
