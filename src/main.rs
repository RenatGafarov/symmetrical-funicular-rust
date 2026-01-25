mod bot;
mod config;
mod domain;
mod exchanges;
mod notification;
mod storage;

use bot::Bot;
use config::Config;
use exchanges::poloniex::{Client, ClientConfig, WebSocketManager, PoloniexExchange};
use exchanges::Exchange;
use std::env;
use tracing::{Level, debug, error, info};
use tracing_subscriber::{EnvFilter, fmt};

const DEFAULT_CONFIG_PATH: &str = "configs/config.yaml";

fn parse_config_path() -> String {
    for arg in env::args().skip(1) {
        if let Some(path) = arg.strip_prefix("--config=") {
            return path.to_string();
        }
    }
    DEFAULT_CONFIG_PATH.to_string()
}

fn init_tracing(log_level: Option<&str>) {
    let level = match log_level {
        Some("debug") => Level::DEBUG,
        Some("info") => Level::INFO,
        Some("warn") | Some("warning") => Level::WARN,
        Some("error") => Level::ERROR,
        Some("trace") => Level::TRACE,
        _ => Level::INFO,
    };

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level.to_string()));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .init();
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    // Check for test mode
    if env::args().any(|arg| arg == "--test-ws") {
        test_poloniex_ws().await;
        return;
    }

    if env::args().any(|arg| arg == "--test-client") {
        test_poloniex_client().await;
        return;
    }

    if env::args().any(|arg| arg == "--test-exchange") {
        test_poloniex_exchange().await;
        return;
    }

    // Initialize tracing early so we can see logs from bot initialization
    init_tracing(Some("info"));

    let config_path = parse_config_path();

    let bot = match Bot::from_config_path(&config_path).await {
        Ok(bot) => bot,
        Err(e) => {
            eprintln!("Failed to create bot: {}", e);
            return;
        }
    };

    info!(config = %config_path, "Bot initialized");

    if let Err(e) = bot.start().await {
        error!(error = %e, "Bot error");
    }

    let _ = bot.stop().await;
}

/// Test function for Poloniex WebSocket manager.
async fn test_poloniex_ws() {
    let config_path = parse_config_path();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            return;
        }
    };

    init_tracing(config.app.log_level.as_deref());

    let poloniex_config = match config.exchanges.get("poloniex") {
        Some(c) => c,
        None => {
            error!("Poloniex exchange not found in config");
            return;
        }
    };

    let pairs = config.pairs.clone();
    let (manager, mut orderbooks_rx) = WebSocketManager::new(poloniex_config, pairs);
    let manager = std::sync::Arc::new(manager);

    info!("Starting Poloniex WebSocket...");

    // Spawn subscribe which does: connect + subscribe + ping_loop + read_loop
    let manager_clone = std::sync::Arc::clone(&manager);
    tokio::spawn(async move {
        if let Err(e) = manager_clone.subscribe().await {
            error!(error = %e, "WebSocket error");
        }
    });

    // Receive orderbooks from the channel
    info!("Listening for orderbook updates (press Ctrl+C to stop)...");

    let mut count = 0;
    while let Some(orderbook) = orderbooks_rx.recv().await {
        info!(
            pair = %orderbook.pair,
            best_bid = ?orderbook.best_bid().map(|l| l.price),
            best_ask = ?orderbook.best_ask().map(|l| l.price),
            spread = ?orderbook.spread(),
            "Orderbook update"
        );

        count += 1;
        if count >= 100 {
            break;
        }
    }

    info!("Test completed, received {} orderbooks", count);
}

/// Test function for Poloniex HTTP client.
async fn test_poloniex_client() {
    let config_path = parse_config_path();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            return;
        }
    };

    init_tracing(config.app.log_level.as_deref());

    let poloniex_config = match config.exchanges.get("poloniex") {
        Some(c) => c,
        None => {
            error!("Poloniex exchange not found in config");
            return;
        }
    };

    let client_config = ClientConfig::new(
        poloniex_config.api_key.clone(),
        poloniex_config.api_secret.clone(),
        // poloniex_config.rate_limit.unwrap_or(200) as i64,
        2
    );

    let client = Client::new(client_config);

    info!("Testing Poloniex HTTP client...");

    info!(
        "Rate limit: {}/{}",
        client.request_count(),
        client.rate_limit()
    );

    match client.ping().await {
        Ok(()) => info!("Ping successful"),
        Err(e) => {
            error!(error = %e, "Ping failed");
            return;
        }
    }

    info!(
        "Rate limit: {}/{}",
        client.request_count(),
        client.rate_limit()
    );

    match client.get_server_time().await {
        Ok(time) => info!(server_time = %time, "Server time received"),
        Err(e) => error!(error = %e, "Failed to get server time"),
    }

    info!(
        "Rate limit: {}/{}",
        client.request_count(),
        client.rate_limit()
    );

    tokio::time::sleep(
        std::time::Duration::from_secs(6)
    ).await;

    info!(
        "Rate limit: {}/{}",
        client.request_count(),
        client.rate_limit()
    );

    match client.get_server_time().await {
        Ok(time) => info!(server_time = %time, "Server time received"),
        Err(e) => error!(error = %e, "Failed to get server time"),
    }

    info!(
        "Rate limit: {}/{}",
        client.request_count(),
        client.rate_limit()
    );

    match client.get_server_time().await {
        Ok(time) => info!(server_time = %time, "Server time received"),
        Err(e) => error!(error = %e, "Failed to get server time"),
    }

    info!(
        "Rate limit: {}/{}",
        client.request_count(),
        client.rate_limit()
    );

    info!("Balance testing");

    info!("Test completed");
}

async fn test_poloniex_exchange() {
    let config_path = parse_config_path();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            return;
        }
    };
    init_tracing(config.app.log_level.as_deref());

    info!("Testing Poloniex exchange...");

    let exchange = PoloniexExchange::from_config(&config).unwrap();

    info!("Exchange name: {}", exchange.name().to_string());

    match exchange.connect().await {
        Ok(_) => info!("Exchange connected"),
        Err(e) => {
            error!("Exchange connection failed: {}", e);
            return;
        }
    }

    match exchange.get_balances().await {
        Ok(balances) => {
            info!("Balances received: {} currencies", balances.len());
            for (currency, amount) in &balances {
                debug!(currency = %currency, amount = %amount, "balance");
            }
        }
        Err(e) => error!("Failed to get balances: {}", e),
    }

    match exchange.disconnect().await {
        Ok(_) => info!("Exchange disconnected"),
        Err(e) => error!("Exchange disconnection failed: {}", e)
    }

    info!("Test completed");
}

