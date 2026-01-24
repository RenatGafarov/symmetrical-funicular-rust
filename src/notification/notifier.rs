#![allow(dead_code)]

use chrono::{DateTime, Utc};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

/// Тип события уведомления
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventType {
    /// Обнаружена арбитражная возможность
    Opportunity,
    /// Выполнена сделка
    Execution,
    /// Произошла ошибка
    Error,
    /// Бот запущен
    Startup,
    /// Бот остановлен
    Shutdown,
    /// Периодический обзор статистики
    Overview,
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventType::Opportunity => write!(f, "opportunity"),
            EventType::Execution => write!(f, "execution"),
            EventType::Error => write!(f, "error"),
            EventType::Startup => write!(f, "startup"),
            EventType::Shutdown => write!(f, "shutdown"),
            EventType::Overview => write!(f, "overview"),
        }
    }
}

/// Данные об арбитражной возможности
#[derive(Debug, Clone)]
pub struct OpportunityData {
    pub pair: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub spread_percent: f64,
    pub potential_profit: f64,
    pub quantity: f64,
}

/// Данные о выполнении сделки
#[derive(Debug, Clone)]
pub struct ExecutionData {
    pub pair: String,
    pub buy_exchange: String,
    pub sell_exchange: String,
    pub success: bool,
    pub actual_profit: f64,
    pub execution_time: Duration,
    pub error_message: Option<String>,
}

/// Данные об ошибке
#[derive(Debug, Clone)]
pub struct ErrorData {
    pub component: String,
    pub message: String,
    pub error: Option<String>,
}

/// Данные о запуске бота
#[derive(Debug, Clone)]
pub struct StartupData {
    pub version: String,
    pub exchanges: Vec<String>,
    pub pairs: Vec<String>,
    pub dry_run: bool,
}

/// Данные об остановке бота
#[derive(Debug, Clone)]
pub struct ShutdownData {
    pub reason: String,
    pub uptime: Duration,
    pub graceful: bool,
}

/// Данные периодического обзора
#[derive(Debug, Clone)]
pub struct OverviewData {
    pub uptime: Duration,
    pub detection_cycles: u64,
    pub opportunities_detected: u64,
    pub opportunities_executed: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub total_profit: f64,
    pub dry_run: bool,
}

/// Данные события
#[derive(Debug, Clone)]
pub enum EventData {
    Opportunity(OpportunityData),
    Execution(ExecutionData),
    Error(ErrorData),
    Startup(StartupData),
    Shutdown(ShutdownData),
    Overview(OverviewData),
}

/// Событие уведомления
#[derive(Debug, Clone)]
pub struct Event {
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub data: EventData,
}

impl Event {
    pub fn new(event_type: EventType, data: EventData) -> Self {
        Self {
            event_type,
            timestamp: Utc::now(),
            data,
        }
    }

    pub fn opportunity(data: OpportunityData) -> Self {
        Self::new(EventType::Opportunity, EventData::Opportunity(data))
    }

    pub fn execution(data: ExecutionData) -> Self {
        Self::new(EventType::Execution, EventData::Execution(data))
    }

    pub fn error(data: ErrorData) -> Self {
        Self::new(EventType::Error, EventData::Error(data))
    }

    pub fn startup(data: StartupData) -> Self {
        Self::new(EventType::Startup, EventData::Startup(data))
    }

    pub fn shutdown(data: ShutdownData) -> Self {
        Self::new(EventType::Shutdown, EventData::Shutdown(data))
    }

    pub fn overview(data: OverviewData) -> Self {
        Self::new(EventType::Overview, EventData::Overview(data))
    }
}

/// Трейт для отправки уведомлений
#[async_trait::async_trait]
pub trait Notifier: Send + Sync {
    /// Отправить уведомление синхронно
    async fn send(&self, event: &Event) -> Result<(), NotificationError>;

    /// Отправить уведомление асинхронно (без блокировки)
    fn send_async(&self, event: Event);

    /// Проверить, включены ли уведомления для данного типа событий
    fn is_enabled(&self, event_type: EventType) -> bool;

    /// Закрыть notifier
    async fn close(&self) -> Result<(), NotificationError>;
}

/// Ошибка уведомления
#[derive(Debug, Clone)]
pub struct NotificationError {
    pub message: String,
}

impl fmt::Display for NotificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NotificationError: {}", self.message)
    }
}

impl std::error::Error for NotificationError {}

impl NotificationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// MultiNotifier отправляет уведомления нескольким notifier'ам
pub struct MultiNotifier {
    notifiers: Vec<Arc<dyn Notifier>>,
}

impl MultiNotifier {
    pub fn new(notifiers: Vec<Arc<dyn Notifier>>) -> Self {
        Self { notifiers }
    }
}

#[async_trait::async_trait]
impl Notifier for MultiNotifier {
    async fn send(&self, event: &Event) -> Result<(), NotificationError> {
        let mut errors = Vec::new();
        for notifier in &self.notifiers {
            if notifier.is_enabled(event.event_type) {
                if let Err(e) = notifier.send(event).await {
                    errors.push(e.message);
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(NotificationError::new(errors.join("; ")))
        }
    }

    fn send_async(&self, event: Event) {
        for notifier in &self.notifiers {
            if notifier.is_enabled(event.event_type) {
                notifier.send_async(event.clone());
            }
        }
    }

    fn is_enabled(&self, event_type: EventType) -> bool {
        self.notifiers.iter().any(|n| n.is_enabled(event_type))
    }

    async fn close(&self) -> Result<(), NotificationError> {
        let mut errors = Vec::new();
        for notifier in &self.notifiers {
            if let Err(e) = notifier.close().await {
                errors.push(e.message);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(NotificationError::new(errors.join("; ")))
        }
    }
}

/// NoopNotifier - пустая реализация для тестов
pub struct NoopNotifier;

impl NoopNotifier {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NoopNotifier {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Notifier for NoopNotifier {
    async fn send(&self, _event: &Event) -> Result<(), NotificationError> {
        Ok(())
    }

    fn send_async(&self, _event: Event) {}

    fn is_enabled(&self, _event_type: EventType) -> bool {
        false
    }

    async fn close(&self) -> Result<(), NotificationError> {
        Ok(())
    }
}

// === Функции форматирования ===

/// Форматирует арбитражную возможность
pub fn format_opportunity(data: &OpportunityData) -> String {
    let base = parse_pair_base(&data.pair);
    let pair_tag = format_pair_tag(&data.pair);

    format!(
        "🔔 *Арбитражная возможность*\n\n\
         💰 Спред: *{:.2}%*\n\
         📈 Потенциальная прибыль: *${:.2}*\n\n\
         Пара: {} #{}\n\
         Покупка: {} @ ${:.4}\n\
         Продажа: {} @ ${:.4}\n\
         Количество: {:.6} {}\n\n\
         ⏰ {}",
        data.spread_percent,
        data.potential_profit,
        data.pair,
        pair_tag,
        data.buy_exchange,
        data.buy_price,
        data.sell_exchange,
        data.sell_price,
        data.quantity,
        base,
        Utc::now().format("%H:%M:%S UTC")
    )
}

/// Форматирует результат выполнения сделки
pub fn format_execution(data: &ExecutionData) -> String {
    if data.success {
        format!(
            "✅ *Сделка выполнена*\n\n\
             Пара: {}\n\
             {} → {}\n\
             Прибыль: *${:.2}*\n\
             Время исполнения: {}\n\n\
             ⏰ {}",
            data.pair,
            data.buy_exchange,
            data.sell_exchange,
            data.actual_profit,
            format_duration(data.execution_time),
            Utc::now().format("%H:%M:%S UTC")
        )
    } else {
        format!(
            "❌ *Сделка не выполнена*\n\n\
             Пара: {}\n\
             {} → {}\n\
             Ошибка: {}\n\
             Время: {}\n\n\
             ⏰ {}",
            data.pair,
            data.buy_exchange,
            data.sell_exchange,
            data.error_message
                .as_deref()
                .unwrap_or("Неизвестная ошибка"),
            format_duration(data.execution_time),
            Utc::now().format("%H:%M:%S UTC")
        )
    }
}

/// Форматирует ошибку
pub fn format_error(data: &ErrorData) -> String {
    let error_str = data
        .error
        .as_ref()
        .map(|e| format!("\nОшибка: {}", e))
        .unwrap_or_default();

    format!(
        "⚠️ *Ошибка*\n\n\
         Компонент: {}\n\
         Сообщение: {}{}\n\n\
         ⏰ {}",
        data.component,
        data.message,
        error_str,
        Utc::now().format("%H:%M:%S UTC")
    )
}

/// Форматирует запуск бота
pub fn format_startup(data: &StartupData) -> String {
    let mode = if data.dry_run {
        "🧪 DRY RUN"
    } else {
        "🚀 LIVE"
    };

    format!(
        "🤖 *Бот запущен*\n\n\
         Версия: {}\n\
         Режим: {}\n\
         Биржи: {}\n\
         Пары: {}\n\n\
         ⏰ {}",
        data.version,
        mode,
        data.exchanges.join(", "),
        data.pairs.join(", "),
        Utc::now().format("%H:%M:%S UTC")
    )
}

/// Форматирует остановку бота
pub fn format_shutdown(data: &ShutdownData) -> String {
    let status = if data.graceful {
        "✅ Graceful"
    } else {
        "⚠️ Forced"
    };

    format!(
        "🛑 *Бот остановлен*\n\n\
         Причина: {}\n\
         Статус: {}\n\
         Время работы: {}\n\n\
         ⏰ {}",
        data.reason,
        status,
        format_duration(data.uptime),
        Utc::now().format("%H:%M:%S UTC")
    )
}

/// Форматирует периодический обзор
pub fn format_overview(data: &OverviewData) -> String {
    let mode = if data.dry_run {
        "🧪 DRY RUN"
    } else {
        "🚀 LIVE"
    };

    format!(
        "📊 *Обзор торговли* {}\n\n\
         ⏱ Время работы: {}\n\
         🔄 Циклов детекции: {}\n\n\
         📈 Обнаружено возможностей: {}\n\
         ✅ Выполнено сделок: {}\n\
         ❌ Неудачных: {}\n\n\
         💰 Общая прибыль: *${:.2}*\n\n\
         ⏰ {}",
        mode,
        format_duration(data.uptime),
        add_thousand_separators(data.detection_cycles),
        data.opportunities_detected,
        data.successful_trades,
        data.failed_trades,
        data.total_profit,
        Utc::now().format("%H:%M:%S UTC")
    )
}

/// Форматирует событие в строку
pub fn format_event(event: &Event) -> String {
    match &event.data {
        EventData::Opportunity(data) => format_opportunity(data),
        EventData::Execution(data) => format_execution(data),
        EventData::Error(data) => format_error(data),
        EventData::Startup(data) => format_startup(data),
        EventData::Shutdown(data) => format_shutdown(data),
        EventData::Overview(data) => format_overview(data),
    }
}

// === Вспомогательные функции ===

/// Извлекает базовую валюту из пары (например, "BTC" из "BTC/USDT")
fn parse_pair_base(pair: &str) -> &str {
    pair.split('/').next().unwrap_or(pair)
}

/// Преобразует пару в формат хэштега (например, "BTC/USDT" -> "BTC\_USDT")
/// Underscore escaped for Telegram Markdown compatibility
fn format_pair_tag(pair: &str) -> String {
    pair.replace('/', "\\_")
}

/// Форматирует длительность
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}с", secs)
    } else if secs < 3600 {
        format!("{}м {}с", secs / 60, secs % 60)
    } else if secs < 86400 {
        format!("{}ч {}м", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}д {}ч", secs / 86400, (secs % 86400) / 3600)
    }
}

/// Добавляет разделители тысяч
fn add_thousand_separators(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
