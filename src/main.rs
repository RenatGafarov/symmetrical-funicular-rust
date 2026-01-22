mod bot;
mod config;
mod notification;

use config::Config;
use notification::{
    Event, Notifier, OpportunityData, ShutdownData, StartupData, TelegramConfig, TelegramNotifier,
};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Загружаем конфигурацию из YAML файла
    let cfg = match Config::load("configs/config.yaml") {
        Ok(cfg) => {
            println!("Config loaded successfully: {}", cfg.app.name);
            cfg
        }
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            return;
        }
    };

    print_all_config(&cfg);

    // Получаем Telegram credentials из конфига
    let (bot_token, chat_id) = match &cfg.notification {
        Some(notification) => match &notification.telegram {
            Some(telegram) if telegram.enabled => {
                if telegram.bot_token.is_empty() || telegram.chat_id.is_empty() {
                    eprintln!("Error: TELEGRAM_BOT_TOKEN or TELEGRAM_CHAT_ID not set");
                    return;
                }
                (telegram.bot_token.clone(), telegram.chat_id.clone())
            }
            _ => {
                eprintln!("Error: Telegram notifications not enabled in config");
                return;
            }
        },
        None => {
            eprintln!("Error: notification section not found in config");
            return;
        }
    };

    // Создаем TelegramNotifier
    let telegram_config = TelegramConfig::new(bot_token, chat_id);
    println!("\nTelegramConfig created: {:?}", telegram_config);

    let notifier = match TelegramNotifier::new(telegram_config) {
        Ok(notifier) => {
            println!("TelegramNotifier created successfully");
            notifier
        }
        Err(e) => {
            eprintln!("TelegramNotifier error: {}", e);
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
        exchanges,
        pairs: cfg.pairs.clone(),
        dry_run: cfg.app.env == "development",
    });

    println!("Sending startup event...");
    if let Err(e) = notifier.send(&startup_event).await {
        eprintln!("Error: {}", e);
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

    println!("Sending opportunity event...");
    notifier.send_async(opportunity_event);

    // Пример: событие остановки бота
    let shutdown_event = Event::shutdown(ShutdownData {
        reason: "User requested".to_string(),
        uptime: Duration::from_secs(3600),
        graceful: true,
    });

    println!("Sending shutdown event...");
    if let Err(e) = notifier.send(&shutdown_event).await {
        eprintln!("Error: {}", e);
    }

    // Закрываем notifier
    let _ = notifier.close().await;
}

fn print_all_config(cfg: &Config) {
    println!("\n========== CONFIG ==========");

    // App
    println!("\n[app]");
    println!("  name: {}", cfg.app.name);
    println!("  env: {}", cfg.app.env);
    println!("  log_level: {:?}", cfg.app.log_level);

    // Exchanges
    println!("\n[exchanges]");
    for (name, ex) in &cfg.exchanges {
        println!("  [{}]", name);
        println!("    enabled: {}", ex.enabled);
        println!("    testnet: {}", ex.testnet);
        println!(
            "    api_key: {}",
            if ex.api_key.is_empty() {
                "<not set>"
            } else {
                "<set>"
            }
        );
        println!(
            "    api_secret: {}",
            if ex.api_secret.is_empty() {
                "<not set>"
            } else {
                "<set>"
            }
        );
        println!("    fee_taker: {:?}", ex.fee_taker);
        println!("    rate_limit: {:?}", ex.rate_limit);
        if let Some(ws) = &ex.websocket {
            println!("    [websocket]");
            println!("      enabled: {}", ws.enabled);
            println!("      ping_interval: {:?}", ws.ping_interval);
            println!("      reconnect_delay: {:?}", ws.reconnect_delay);
        }
    }

    // Pairs
    println!("\n[pairs]");
    for pair in &cfg.pairs {
        println!("  - {}", pair);
    }

    // Orderbook
    if let Some(ob) = &cfg.orderbook {
        println!("\n[orderbook]");
        println!("  max_depth: {:?}", ob.max_depth);
        println!("  max_age: {:?}", ob.max_age);
    }

    // Arbitrage
    if let Some(arb) = &cfg.arbitrage {
        println!("\n[arbitrage]");
        println!("  detection_timeout: {:?}", arb.detection_timeout);
        if let Some(ce) = &arb.cross_exchange {
            println!("  [cross_exchange]");
            println!("    min_profit_threshold: {:?}", ce.min_profit_threshold);
            println!("    min_quantity: {:?}", ce.min_quantity);
            println!("    opportunity_ttl: {:?}", ce.opportunity_ttl);
        }
    }

    // Execution
    if let Some(exec) = &cfg.execution {
        println!("\n[execution]");
        println!("  timeout: {:?}", exec.timeout);
        if let Some(retry) = &exec.retry {
            println!("  [retry]");
            println!("    max_attempts: {:?}", retry.max_attempts);
            println!("    initial_delay: {:?}", retry.initial_delay);
            println!("    max_delay: {:?}", retry.max_delay);
            println!("    multiplier: {:?}", retry.multiplier);
        }
    }

    // Risk
    if let Some(risk) = &cfg.risk {
        println!("\n[risk]");
        println!(
            "  max_position_per_exchange: {:?}",
            risk.max_position_per_exchange
        );
        println!("  daily_loss_limit: {:?}", risk.daily_loss_limit);
        println!("  kill_switch_drawdown: {:?}", risk.kill_switch_drawdown);
        println!("  max_open_orders: {:?}", risk.max_open_orders);
    }

    // Notification
    if let Some(notif) = &cfg.notification {
        println!("\n[notification]");
        if let Some(tg) = &notif.telegram {
            println!("  [telegram]");
            println!("    enabled: {}", tg.enabled);
            println!(
                "    bot_token: {}",
                if tg.bot_token.is_empty() {
                    "<not set>"
                } else {
                    "<set>"
                }
            );
            println!(
                "    chat_id: {}",
                if tg.chat_id.is_empty() {
                    "<not set>"
                } else {
                    "<set>"
                }
            );
            println!(
                "    error_chat_id: {}",
                if tg.error_chat_id.is_empty() {
                    "<not set>"
                } else {
                    "<set>"
                }
            );
            println!("    notify_opportunities: {}", tg.notify_opportunities);
            println!("    notify_executions: {}", tg.notify_executions);
            println!("    notify_errors: {}", tg.notify_errors);
            println!("    notify_overview: {}", tg.notify_overview);
            println!("    overview_interval: {:?}", tg.overview_interval);
        }
    }

    // Storage
    if let Some(storage) = &cfg.storage {
        println!("\n[storage]");
        println!("  enabled: {}", storage.enabled);
        println!("  path: {:?}", storage.path);
    }

    // Balance
    if let Some(balance) = &cfg.balance {
        println!("\n[balance]");
        println!("  enabled: {}", balance.enabled);
        println!("  sync_interval: {:?}", balance.sync_interval);
        println!("  max_age: {:?}", balance.max_age);
        println!("  sync_after_trade: {}", balance.sync_after_trade);
    }

    println!("\n============================\n");
}
