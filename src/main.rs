mod bot;
mod config;
mod notification;

use config::Config;
use notification::{
    Event, Notifier, OpportunityData, ShutdownData, StartupData, TelegramConfig, TelegramNotifier,
};
use std::time::Duration;
use tracing::{debug, error, info, warn, Level};
use tracing_subscriber::{fmt, EnvFilter};

fn init_tracing(log_level: Option<&str>) {
    let level = match log_level {
        Some("debug") => Level::DEBUG,
        Some("info") => Level::INFO,
        Some("warn") | Some("warning") => Level::WARN,
        Some("error") => Level::ERROR,
        Some("trace") => Level::TRACE,
        _ => Level::INFO,
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level.to_string()));

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
    // Загружаем .env для получения переменных окружения
    dotenvy::dotenv().ok();

    // Загружаем конфигурацию из YAML файла
    let cfg = match Config::load("configs/config.yaml") {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            return;
        }
    };

    // Инициализируем логгер на основе конфигурации
    init_tracing(cfg.app.log_level.as_deref());

    info!(app = %cfg.app.name, env = %cfg.app.env, "Config loaded successfully");

    if tracing::enabled!(Level::DEBUG) {
        print_all_config(&cfg);
    }

    // Получаем Telegram credentials из конфига
    let (bot_token, chat_id) = match &cfg.notification {
        Some(notification) => match &notification.telegram {
            Some(telegram) if telegram.enabled => {
                if telegram.bot_token.is_empty() || telegram.chat_id.is_empty() {
                    error!("TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID not set");
                    return;
                }
                (telegram.bot_token.clone(), telegram.chat_id.clone())
            }
            _ => {
                warn!("Telegram notifications not enabled in config");
                return;
            }
        },
        None => {
            warn!("Notification section not found in config");
            return;
        }
    };

    // Создаем TelegramNotifier
    let telegram_config = TelegramConfig::new(bot_token, chat_id);
    debug!(?telegram_config, "TelegramConfig created");

    let notifier = match TelegramNotifier::new(telegram_config) {
        Ok(notifier) => {
            info!("TelegramNotifier created successfully");
            notifier
        }
        Err(e) => {
            error!(error = %e, "Failed to create TelegramNotifier");
            return;
        }
    };

    // Собираем список включённых бирж из конфига
    let exchanges: Vec<String> = cfg
        .exchanges
        .iter()
        .filter(|(_, ex)| ex.enabled)
        .map(|(name, _)| name.clone())
        .collect();

    // Пример: событие запуска бота
    let startup_event = Event::startup(StartupData {
        version: "0.1.0".to_string(),
        exchanges: exchanges.clone(),
        pairs: cfg.pairs.clone(),
        dry_run: cfg.app.env == "development",
    });

    info!(exchanges = ?exchanges, pairs = ?cfg.pairs, "Sending startup event");
    if let Err(e) = notifier.send(&startup_event).await {
        error!(error = %e, "Failed to send startup event");
    }

    // Пример: событие обнаружения арбитражной возможности
    let opportunity_event = Event::opportunity(OpportunityData {
        pair: "BTC/USDT".to_string(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "Bybit".to_string(),
        buy_price: 42000.50,
        sell_price: 42150.75,
        spread_percent: 0.36,
        potential_profit: 15.025,
        quantity: 0.1,
    });

    info!(pair = "BTC/USDT", spread = 0.36, "Sending opportunity event");
    notifier.send_async(opportunity_event);

    // Пример: событие остановки бота
    let shutdown_event = Event::shutdown(ShutdownData {
        reason: "User requested".to_string(),
        uptime: Duration::from_secs(3600),
        graceful: true,
    });

    info!(reason = "User requested", graceful = true, "Sending shutdown event");
    if let Err(e) = notifier.send(&shutdown_event).await {
        error!(error = %e, "Failed to send shutdown event");
    }

    // Закрываем notifier
    let _ = notifier.close().await;
    info!("Bot shutdown complete");
}

fn print_all_config(cfg: &Config) {
    debug!("========== CONFIG ==========");

    // App
    debug!(
        name = %cfg.app.name,
        env = %cfg.app.env,
        log_level = ?cfg.app.log_level,
        "[app]"
    );

    // Exchanges
    for (name, ex) in &cfg.exchanges {
        debug!(
            exchange = %name,
            enabled = ex.enabled,
            testnet = ex.testnet,
            api_key = if ex.api_key.is_empty() { "<not set>" } else { "<set>" },
            api_secret = if ex.api_secret.is_empty() { "<not set>" } else { "<set>" },
            fee_taker = ?ex.fee_taker,
            rate_limit = ?ex.rate_limit,
            "[exchange]"
        );
        if let Some(ws) = &ex.websocket {
            debug!(
                enabled = ws.enabled,
                ping_interval = ?ws.ping_interval,
                reconnect_delay = ?ws.reconnect_delay,
                "[exchange.websocket]"
            );
        }
    }

    // Pairs
    debug!(pairs = ?cfg.pairs, "[pairs]");

    // Orderbook
    if let Some(ob) = &cfg.orderbook {
        debug!(
            max_depth = ?ob.max_depth,
            max_age = ?ob.max_age,
            "[orderbook]"
        );
    }

    // Arbitrage
    if let Some(arb) = &cfg.arbitrage {
        debug!(detection_timeout = ?arb.detection_timeout, "[arbitrage]");
        if let Some(ce) = &arb.cross_exchange {
            debug!(
                min_profit_threshold = ?ce.min_profit_threshold,
                min_quantity = ?ce.min_quantity,
                opportunity_ttl = ?ce.opportunity_ttl,
                "[arbitrage.cross_exchange]"
            );
        }
    }

    // Execution
    if let Some(exec) = &cfg.execution {
        debug!(timeout = ?exec.timeout, "[execution]");
        if let Some(retry) = &exec.retry {
            debug!(
                max_attempts = ?retry.max_attempts,
                initial_delay = ?retry.initial_delay,
                max_delay = ?retry.max_delay,
                multiplier = ?retry.multiplier,
                "[execution.retry]"
            );
        }
    }

    // Risk
    if let Some(risk) = &cfg.risk {
        debug!(
            max_position_per_exchange = ?risk.max_position_per_exchange,
            daily_loss_limit = ?risk.daily_loss_limit,
            kill_switch_drawdown = ?risk.kill_switch_drawdown,
            max_open_orders = ?risk.max_open_orders,
            "[risk]"
        );
    }

    // Notification
    if let Some(notif) = &cfg.notification {
        if let Some(tg) = &notif.telegram {
            debug!(
                enabled = tg.enabled,
                bot_token = if tg.bot_token.is_empty() { "<not set>" } else { "<set>" },
                chat_id = if tg.chat_id.is_empty() { "<not set>" } else { "<set>" },
                error_chat_id = if tg.error_chat_id.is_empty() { "<not set>" } else { "<set>" },
                notify_opportunities = tg.notify_opportunities,
                notify_executions = tg.notify_executions,
                notify_errors = tg.notify_errors,
                notify_overview = tg.notify_overview,
                overview_interval = ?tg.overview_interval,
                "[notification.telegram]"
            );
        }
    }

    // Storage
    if let Some(storage) = &cfg.storage {
        debug!(
            enabled = storage.enabled,
            path = ?storage.path,
            "[storage]"
        );
    }

    // Balance
    if let Some(balance) = &cfg.balance {
        debug!(
            enabled = balance.enabled,
            sync_interval = ?balance.sync_interval,
            max_age = ?balance.max_age,
            sync_after_trade = balance.sync_after_trade,
            "[balance]"
        );
    }

    debug!("============================");
}
