mod bot;
mod config;
mod domain;
mod notification;
mod storage;

use bot::Bot;
use std::env;
use tracing::{error, info, Level};
use tracing_subscriber::{fmt, EnvFilter};

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
