//! Main arbitrage bot implementation.
//!
//! Coordinates all components: exchanges, detector, executor, risk manager, and notifications.

mod error;
mod stats;

pub use error::BotError;
pub use stats::Stats;

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::domain::Opportunity;
use crate::notification::{
    Event, Notifier, OverviewData, ShutdownData, StartupData, TelegramConfig, TelegramNotifier,
};
use crate::storage::{OpportunityStorage, SqliteStorage, SqliteStorageConfig};

/// Main arbitrage bot that coordinates all components.
pub struct Bot {
    cfg: Config,
    notifier: Option<Arc<TelegramNotifier>>,
    storage: Option<Arc<SqliteStorage>>,

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
    /// Creates a new Bot instance from config file path.
    pub async fn from_config_path(config_path: &str) -> Result<Self, BotError> {
        let cfg = Config::load(config_path)?;

        let dry_run = cfg.app.env == "development";

        // Set detection timeout from config or use default
        let detection_timeout = cfg
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
            cfg: cfg.clone(),
            notifier: None,
            storage: None,
            detection_timeout,
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_time: "".to_string(),
            dry_run,
            started_at: Mutex::new(None),
            running: Mutex::new(false),
            stats: Mutex::new(Stats::default()),
            executing_pairs: RwLock::new(HashSet::new()),
        };

        // Create notifier if configured
        if let Some(ref notification) = cfg.notification {
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

        // Create storage if configured
        if let Some(ref storage_cfg) = cfg.storage {
            debug!(enabled = storage_cfg.enabled, "Storage configuration found");
            if storage_cfg.enabled {
                let path = storage_cfg
                    .path
                    .clone()
                    .unwrap_or_else(|| "opportunities.db".to_string());

                debug!(path = %path, "Initializing storage");

                let config = SqliteStorageConfig {
                    path: path.clone(),
                    max_connections: 5,
                };

                match SqliteStorage::new(config).await {
                    Ok(storage) => {
                        bot.storage = Some(Arc::new(storage));
                        info!(path = %path, "Storage initialized");
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to create storage");
                    }
                }
            }
        } else {
            debug!("No storage configuration found");
        }

        Ok(bot)
    }

    /// Returns the log level from config.
    pub fn log_level(&self) -> Option<&str> {
        self.cfg.app.log_level.as_deref()
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

        // Close storage
        if let Some(ref storage) = self.storage {
            if let Err(e) = storage.close().await {
                warn!(error = %e, "Failed to close storage");
            } else {
                info!("Storage closed");
            }
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

        let mut last_overview = Instant::now();

        info!(
            detection_interval = ?Duration::from_millis(500),
            overview_interval = ?overview_interval,
            detection_timeout = ?self.detection_timeout,
            "Starting main detection loop"
        );

        loop {
            interval.tick().await;

            if !self.is_running().await {
                break;
            }

            self.detect_and_execute().await;

            // Check if it's time for overview
            if last_overview.elapsed() >= overview_interval {
                self.send_overview().await;
                last_overview = Instant::now();
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

    /// Saves an opportunity to storage if storage is enabled.
    /// Returns true if the opportunity was saved (new), false if it already exists or storage is disabled.
    pub async fn save_opportunity(&self, opportunity: &Opportunity) -> bool {
        if let Some(ref storage) = self.storage {
            match storage.save(opportunity).await {
                Ok(saved) => {
                    if saved {
                        debug!(
                            id = %opportunity.id,
                            pair = %opportunity.pair,
                            "Opportunity saved to storage"
                        );

                        // Update stats
                        let mut stats = self.stats.lock().await;
                        stats.opportunities_detected += 1;
                    }
                    saved
                }
                Err(e) => {
                    warn!(
                        error = %e,
                        id = %opportunity.id,
                        "Failed to save opportunity to storage"
                    );
                    false
                }
            }
        } else {
            false
        }
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

        info!(
            uptime = ?uptime,
            detection_cycles = stats.detection_cycles,
            "Sending overview notification"
        );

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
