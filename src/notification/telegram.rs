#![allow(dead_code)]

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::error;

use crate::notification::{
    Event, EventType, NotificationError, Notifier, format_event,
};

const TELEGRAM_API_URL: &str = "https://api.telegram.org/bot";
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_MESSAGE_LENGTH: usize = 4096;
const ASYNC_QUEUE_SIZE: usize = 100;

/// Конфигурация Telegram notifier
#[derive(Debug, Clone)]
pub struct TelegramConfig {
    /// Токен бота от BotFather
    pub bot_token: String,
    /// ID чата для отправки уведомлений
    pub chat_id: String,
    /// Опциональный ID чата для ошибок
    pub error_chat_id: Option<String>,
    /// Включить уведомления о возможностях
    pub notify_opportunities: bool,
    /// Включить уведомления о выполнении
    pub notify_executions: bool,
    /// Включить уведомления об ошибках
    pub notify_errors: bool,
    /// Включить периодические обзоры
    pub notify_overview: bool,
}

impl TelegramConfig {
    pub fn new(bot_token: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            chat_id: chat_id.into(),
            error_chat_id: None,
            notify_opportunities: true,
            notify_executions: true,
            notify_errors: true,
            notify_overview: true,
        }
    }

    pub fn with_error_chat_id(mut self, chat_id: impl Into<String>) -> Self {
        self.error_chat_id = Some(chat_id.into());
        self
    }
}

/// Telegram Notifier для отправки уведомлений через Telegram Bot API
pub struct TelegramNotifier {
    config: TelegramConfig,
    http_client: reqwest::Client,
    api_url: String,
    sender: mpsc::Sender<Event>,
    shutdown: Arc<Mutex<bool>>,
}

impl TelegramNotifier {
    /// Создает новый TelegramNotifier
    pub fn new(config: TelegramConfig) -> Result<Self, NotificationError> {
        if config.bot_token.is_empty() {
            return Err(NotificationError::new("bot_token is required"));
        }
        if config.chat_id.is_empty() {
            return Err(NotificationError::new("chat_id is required"));
        }

        let http_client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .build()
            .map_err(|e| NotificationError::new(format!("Failed to create HTTP client: {}", e)))?;

        let api_url = format!("{}{}", TELEGRAM_API_URL, config.bot_token);

        let (sender, receiver) = mpsc::channel(ASYNC_QUEUE_SIZE);
        let shutdown = Arc::new(Mutex::new(false));

        let notifier = Self {
            config: config.clone(),
            http_client: http_client.clone(),
            api_url: api_url.clone(),
            sender,
            shutdown: shutdown.clone(),
        };

        // Запускаем воркер для асинхронной обработки
        Self::spawn_worker(receiver, config, http_client, api_url, shutdown);

        Ok(notifier)
    }

    fn spawn_worker(
        mut receiver: mpsc::Receiver<Event>,
        config: TelegramConfig,
        http_client: reqwest::Client,
        api_url: String,
        shutdown: Arc<Mutex<bool>>,
    ) {
        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                let chat_id = if event.event_type == EventType::Error {
                    config.error_chat_id.as_ref().unwrap_or(&config.chat_id)
                } else {
                    &config.chat_id
                };

                let text = format_event(&event);
                if let Err(e) = Self::send_message_to_chat_static(
                    &http_client,
                    &api_url,
                    chat_id,
                    &text,
                )
                .await
                {
                    error!(error = %e, "Failed to send Telegram message");
                }
            }

            // Помечаем воркер как завершенный
            let mut is_shutdown = shutdown.lock().await;
            *is_shutdown = true;
        });
    }

    async fn send_message_to_chat(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<(), NotificationError> {
        Self::send_message_to_chat_static(&self.http_client, &self.api_url, chat_id, text).await
    }

    async fn send_message_to_chat_static(
        http_client: &reqwest::Client,
        api_url: &str,
        chat_id: &str,
        text: &str,
    ) -> Result<(), NotificationError> {
        // Обрезаем сообщение если превышает лимит Telegram
        let text = if text.len() > MAX_MESSAGE_LENGTH {
            &text[..MAX_MESSAGE_LENGTH]
        } else {
            text
        };

        let url = format!("{}/sendMessage", api_url);

        let payload = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "Markdown"
        });

        let response = http_client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| NotificationError::new(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(NotificationError::new(format!(
                "Telegram API error: {} - {}",
                status, body
            )));
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Notifier for TelegramNotifier {
    async fn send(&self, event: &Event) -> Result<(), NotificationError> {
        if !self.is_enabled(event.event_type) {
            return Ok(());
        }

        let chat_id = if event.event_type == EventType::Error {
            self.config
                .error_chat_id
                .as_ref()
                .unwrap_or(&self.config.chat_id)
        } else {
            &self.config.chat_id
        };

        let text = format_event(event);
        self.send_message_to_chat(chat_id, &text).await
    }

    fn send_async(&self, event: Event) {
        if !self.is_enabled(event.event_type) {
            return;
        }

        if let Err(e) = self.sender.try_send(event) {
            error!(error = %e, "Failed to queue Telegram message");
        }
    }

    fn is_enabled(&self, event_type: EventType) -> bool {
        match event_type {
            EventType::Startup | EventType::Shutdown => true,
            EventType::Opportunity => self.config.notify_opportunities,
            EventType::Execution => self.config.notify_executions,
            EventType::Error => self.config.notify_errors,
            EventType::Overview => self.config.notify_overview,
        }
    }

    async fn close(&self) -> Result<(), NotificationError> {
        // Даем время воркеру обработать оставшиеся сообщения
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}
