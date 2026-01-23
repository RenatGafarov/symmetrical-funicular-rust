//! Main arbitrage bot implementation.
//!
//! Coordinates all components: exchanges, detector, executor, risk manager, and notifications.

mod config;
mod error;
mod stats;

pub use config::BotConfig;
pub use error::BotError;
pub use stats::Stats;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::notification::{
    Event, Notifier, OverviewData, ShutdownData, StartupData, TelegramConfig, TelegramNotifier,
};

/// Main arbitrage bot that coordinates all components.
pub struct Bot {
    cfg: Config,
    notifier: Option<Arc<TelegramNotifier>>,

    // Timeouts
    detection_timeout: Duration,

    // Runtime state
    version: String,
    build_time: String,
    dry_run: bool,
    started_at: Mutex<Option<Instant>>,
    running: Mutex<bool>,
    stats: Mutex<Stats>,

    // Execution lock - prevents parallel executions for the same pair
    executing_pairs: RwLock<HashSet<String>>,
}

impl Bot {
    /// Creates a new Bot instance.
    pub fn new(cfg: BotConfig) -> Result<Self, BotError> {
        // Set detection timeout from config or use default
        let detection_timeout = cfg
            .app_config
            .arbitrage
            .as_ref()
            .and_then(|a| {
                if a.detection_timeout.as_secs() > 0 {
                    Some(a.detection_timeout)
                } else {
                    None
                }
            })
            .unwrap_or(Duration::from_secs(10));

        let mut bot = Bot {
            cfg: cfg.app_config.clone(),
            notifier: None,
            detection_timeout,
            version: cfg.version,
            build_time: cfg.build_time,
            dry_run: cfg.dry_run,
            started_at: Mutex::new(None),
            running: Mutex::new(false),
            stats: Mutex::new(Stats::default()),
            executing_pairs: RwLock::new(HashSet::new()),
        };

        // Create notifier if configured
        if let Some(ref notification) = cfg.app_config.notification {
            if let Some(ref telegram) = notification.telegram {
                if telegram.enabled
                    && !telegram.bot_token.is_empty()
                    && !telegram.chat_id.is_empty()
                {
                    let telegram_config =
                        TelegramConfig::new(telegram.bot_token.clone(), telegram.chat_id.clone());

                    match TelegramNotifier::new(telegram_config) {
                        Ok(notifier) => {
                            bot.notifier = Some(Arc::new(notifier));
                            info!("Telegram notifier created");
                        }
                        Err(e) => {
                            warn!(error = %e, "Failed to create Telegram notifier");
                        }
                    }
                }
            }
        }

        Ok(bot)
    }

    /// Starts the bot and begins arbitrage detection.
    pub async fn start(&self) -> Result<(), BotError> {
        {
            let mut running = self.running.lock().await;
            if *running {
                return Err(BotError::AlreadyRunning);
            }
            *running = true;
        }

        {
            let mut started_at = self.started_at.lock().await;
            *started_at = Some(Instant::now());
        }

        let exchanges = self.get_exchange_names();

        info!(
            version = %self.version,
            build_time = %self.build_time,
            dry_run = self.dry_run,
            exchanges = ?exchanges,
            pairs = ?self.cfg.pairs,
            "Starting arbitrage bot"
        );

        // Send startup notification
        self.send_notification(Event::startup(StartupData {
            version: self.version.clone(),
            exchanges,
            pairs: self.cfg.pairs.clone(),
            dry_run: self.dry_run,
        }))
        .await;

        // Start main loop
        self.run_main_loop().await
    }

    /// Gracefully stops the bot.
    pub async fn stop(&self) -> Result<(), BotError> {
        {
            let mut running = self.running.lock().await;
            if !*running {
                return Ok(());
            }
            *running = false;
        }

        info!("Stopping bot...");

        let uptime = self.uptime().await;

        // Send shutdown notification
        self.send_notification(Event::shutdown(ShutdownData {
            reason: "graceful shutdown".to_string(),
            uptime,
            graceful: true,
        }))
        .await;

        // Close notifier
        if let Some(ref notifier) = self.notifier {
            let _ = notifier.close().await;
        }

        info!(uptime = ?uptime, "Bot stopped");

        Ok(())
    }

    /// Returns a copy of the current statistics.
    pub async fn stats(&self) -> Stats {
        self.stats.lock().await.clone()
    }

    /// Returns true if the bot is currently running.
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// Returns how long the bot has been running.
    pub async fn uptime(&self) -> Duration {
        self.started_at
            .lock()
            .await
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Returns exchange names from config.
    fn get_exchange_names(&self) -> Vec<String> {
        self.cfg
            .exchanges
            .iter()
            .filter(|(_, ex)| ex.enabled)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Main arbitrage detection and execution loop.
    async fn run_main_loop(&self) -> Result<(), BotError> {
        let mut interval = tokio::time::interval(Duration::from_millis(500));

        // Overview interval from config or default 1 hour
        let overview_interval = self
            .cfg
            .notification
            .as_ref()
            .and_then(|n| n.telegram.as_ref())
            .map(|t| t.overview_interval)
            .filter(|d| d.as_secs() > 0)
            .unwrap_or(Duration::from_secs(3600));

        let mut overview_interval_timer = tokio::time::interval(overview_interval);

        info!(
            detection_interval = ?Duration::from_millis(500),
            overview_interval = ?overview_interval,
            detection_timeout = ?self.detection_timeout,
            "Starting main detection loop"
        );

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !self.is_running().await {
                        break;
                    }
                    self.detect_and_execute().await;
                }
                _ = overview_interval_timer.tick() => {
                    if !self.is_running().await {
                        break;
                    }
                    self.send_overview().await;
                }
            }
        }

        Ok(())
    }

    /// Runs one cycle of arbitrage detection and execution.
    async fn detect_and_execute(&self) {
        let cycles = {
            let mut stats = self.stats.lock().await;
            stats.detection_cycles += 1;
            stats.detection_cycles
        };

        // Log every 20 cycles (~10 sec) at Info level
        if cycles % 20 == 1 {
            info!(
                cycle = cycles,
                pairs = self.cfg.pairs.len(),
                "Detection cycle running"
            );
        }

        // TODO: Implement actual detection when components are ready
        for pair in &self.cfg.pairs {
            debug!(pair = %pair, "Processing pair");
        }
    }

    /// Attempts to acquire a lock for executing trades on the given pair.
    pub async fn try_lock_pair(&self, pair: &str) -> bool {
        let mut pairs = self.executing_pairs.write().await;
        if pairs.contains(pair) {
            return false;
        }
        pairs.insert(pair.to_string());
        true
    }

    /// Releases the execution lock for the given pair.
    pub async fn unlock_pair(&self, pair: &str) {
        let mut pairs = self.executing_pairs.write().await;
        pairs.remove(pair);
    }

    /// Sends a notification event if notifier is configured.
    async fn send_notification(&self, event: Event) {
        if let Some(ref notifier) = self.notifier {
            if let Err(e) = notifier.send(&event).await {
                debug!(
                    event_type = %event.event_type,
                    error = %e,
                    "Failed to send notification"
                );
            }
        }
    }

    /// Sends a periodic overview notification with current stats.
    async fn send_overview(&self) {
        let stats = self.stats().await;
        let uptime = self.uptime().await;

        self.send_notification(Event::overview(OverviewData {
            uptime,
            detection_cycles: stats.detection_cycles,
            opportunities_detected: stats.opportunities_detected,
            opportunities_executed: stats.opportunities_executed,
            successful_trades: stats.successful_trades,
            failed_trades: stats.failed_trades,
            total_profit: stats.total_profit,
            dry_run: self.dry_run,
        }))
        .await;
    }
}
