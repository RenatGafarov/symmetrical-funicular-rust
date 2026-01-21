mod bot;
mod config;
mod constants;
mod notification;

use notification::{
    Event, Notifier, OpportunityData, ShutdownData, StartupData, TelegramConfig, TelegramNotifier,
};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // Загружаем конфигурацию из .env
    if let Err(e) = config::load() {
        println!("Warning: {}", e);
    }

    // Создаем TelegramNotifier из переменных окружения
    let bot_token = match config::require(constants::TELEGRAM_BOT_TOKEN) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    let chat_id = match config::require(constants::TELEGRAM_CHAT_ID) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };
    let config = TelegramConfig::new(bot_token, chat_id);
    println!("\nTelegramConfig created: {:?}", config);

    // TelegramNotifier требует валидный токен
    let notifier = match TelegramNotifier::new(config) {
        Ok(notifier) => {
            println!("TelegramNotifier created successfully");
            notifier
        }
        Err(e) => {
            eprintln!("TelegramNotifier error: {}", e);
            return;
        }
    };

    // Пример: событие запуска бота
    let startup_event = Event::startup(StartupData {
        version: "0.1.0".to_string(),
        exchanges: vec!["Binance".to_string(), "Kraken".to_string()],
        pairs: vec!["BTC/USDT".to_string(), "ETH/USDT".to_string()],
        dry_run: true,
    });

    println!("Sending startup event...");
    if let Err(e) = notifier.send(&startup_event).await {
        eprintln!("Error: {}", e);
    }

    // Пример: событие обнаружения арбитражной возможности
    let opportunity_event = Event::opportunity(OpportunityData {
        pair: "BTC/USDT".to_string(),
        buy_exchange: "Binance".to_string(),
        sell_exchange: "Kraken".to_string(),
        buy_price: 42000.50,
        sell_price: 42150.75,
        spread_percent: 0.36,
        potential_profit: 15.025,
        quantity: 0.1,
    });

    println!("Sending opportunity event...");
    notifier.send_async(opportunity_event);

    // Пример: событие остановки бота
    let shutdown_event = Event::shutdown(
        ShutdownData {
            reason: "User requested".to_string(),
            uptime: Duration::from_secs(3600),
            graceful: true,
        }
    );

    println!("Sending shutdown event...");
    if let Err(e) = notifier.send(
        &shutdown_event
    ).await {
        eprintln!("Error: {}", e);
    }

    // Закрываем notifier
    let _ = notifier.close().await;
}
