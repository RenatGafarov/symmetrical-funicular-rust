//! SQLite implementation of OpportunityStorage.

use crate::domain::{Opportunity, OpportunityType};
use crate::storage::{OpportunityStorage, StorageError};
use async_trait::async_trait;
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Row, Sqlite};
use std::str::FromStr;
use tracing::{debug, info};

/// SqliteStorage implements OpportunityStorage using SQLite.
pub struct SqliteStorage {
    pool: Pool<Sqlite>,
}

/// SqliteStorageConfig holds SQLite storage configuration.
#[derive(Debug, Clone)]
pub struct SqliteStorageConfig {
    /// Path to the SQLite database file.
    pub path: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
}

impl Default for SqliteStorageConfig {
    fn default() -> Self {
        Self {
            path: "opportunities.db".to_string(),
            max_connections: 5,
        }
    }
}

impl SqliteStorage {
    /// Creates a new SQLite storage instance.
    pub async fn new(config: SqliteStorageConfig) -> Result<Self, StorageError> {
        let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", config.path))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

        let pool = SqlitePoolOptions::new()
            .max_connections(config.max_connections)
            .connect_with(options)
            .await?;

        let storage = Self { pool };

        storage.migrate().await?;

        info!(path = %config.path, "SQLite storage initialized");
        Ok(storage)
    }

    /// Runs database migrations to create the schema.
    async fn migrate(&self) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS opportunities (
                id TEXT PRIMARY KEY,
                unique_hash TEXT NOT NULL UNIQUE,
                type TEXT NOT NULL,
                pair TEXT NOT NULL,
                buy_exchange TEXT NOT NULL,
                sell_exchange TEXT NOT NULL,
                buy_price TEXT NOT NULL,
                sell_price TEXT NOT NULL,
                quantity TEXT NOT NULL,
                gross_profit TEXT NOT NULL,
                net_profit TEXT NOT NULL,
                profit_percent TEXT NOT NULL,
                buy_fee TEXT NOT NULL,
                sell_fee TEXT NOT NULL,
                detected_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_opportunities_pair ON opportunities(pair)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_opportunities_detected_at ON opportunities(detected_at)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_opportunities_exchanges ON opportunities(buy_exchange, sell_exchange)",
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// Generates a unique hash for detecting duplicate opportunities.
///
/// An opportunity is unique based on: pair, buy_exchange, sell_exchange,
/// profit_percent (rounded to 2 decimals), and a 5-minute time window.
/// This prevents duplicate notifications for the same opportunity within 5 minutes.
fn generate_unique_hash(opp: &Opportunity) -> String {
    // Round profit percent to 2 decimal places (e.g., 0.7234 -> 0.72)
    let profit_rounded = (opp.profit_percent * Decimal::from(100))
        .round_dp(2)
        .to_string();

    // Round detected_at to 5-minute window (e.g., 16:37 and 16:39 both become 16:35)
    let minute = opp.detected_at.minute();
    let window_minutes = (minute / 5) * 5;
    let time_window = opp
        .detected_at
        .with_minute(window_minutes)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();
    let time_window_str = time_window.format("%Y-%m-%dT%H:%M").to_string();

    let data = format!(
        "{}|{}|{}|{}|{}",
        opp.pair, opp.buy_exchange, opp.sell_exchange, profit_rounded, time_window_str
    );

    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    let hash = hasher.finalize();

    // Use first 16 bytes for shorter hash
    hex::encode(&hash[..16])
}

#[async_trait]
impl OpportunityStorage for SqliteStorage {
    async fn save(&self, opp: &Opportunity) -> Result<bool, StorageError> {
        let unique_hash = generate_unique_hash(opp);

        let result = sqlx::query(
            r#"
            INSERT INTO opportunities (
                id, unique_hash, type, pair, buy_exchange, sell_exchange,
                buy_price, sell_price, quantity, gross_profit, net_profit,
                profit_percent, buy_fee, sell_fee, detected_at, expires_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT(unique_hash) DO NOTHING
            "#,
        )
        .bind(&opp.id)
        .bind(&unique_hash)
        .bind(opp.opportunity_type.to_string())
        .bind(&opp.pair)
        .bind(&opp.buy_exchange)
        .bind(&opp.sell_exchange)
        .bind(opp.buy_price.to_string())
        .bind(opp.sell_price.to_string())
        .bind(opp.quantity.to_string())
        .bind(opp.gross_profit.to_string())
        .bind(opp.net_profit.to_string())
        .bind(opp.profit_percent.to_string())
        .bind(opp.buy_fee.to_string())
        .bind(opp.sell_fee.to_string())
        .bind(opp.detected_at.to_rfc3339())
        .bind(opp.expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        let rows_affected = result.rows_affected();

        if rows_affected > 0 {
            debug!(
                id = %opp.id,
                pair = %opp.pair,
                hash = %unique_hash,
                "Opportunity saved"
            );
        }

        Ok(rows_affected > 0)
    }

    async fn get_by_id(&self, id: &str) -> Result<Option<Opportunity>, StorageError> {
        let row = sqlx::query(
            r#"
            SELECT id, type, pair, buy_exchange, sell_exchange, buy_price, sell_price,
                quantity, gross_profit, net_profit, profit_percent, buy_fee, sell_fee,
                detected_at, expires_at
            FROM opportunities WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let opp = parse_opportunity_row(&row)?;
                Ok(Some(opp))
            }
            None => Ok(None),
        }
    }

    async fn get_all(&self) -> Result<Vec<Opportunity>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, type, pair, buy_exchange, sell_exchange, buy_price, sell_price,
                quantity, gross_profit, net_profit, profit_percent, buy_fee, sell_fee,
                detected_at, expires_at
            FROM opportunities ORDER BY detected_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(parse_opportunity_row).collect()
    }

    async fn get_by_pair(&self, pair: &str) -> Result<Vec<Opportunity>, StorageError> {
        let rows = sqlx::query(
            r#"
            SELECT id, type, pair, buy_exchange, sell_exchange, buy_price, sell_price,
                quantity, gross_profit, net_profit, profit_percent, buy_fee, sell_fee,
                detected_at, expires_at
            FROM opportunities WHERE pair = ? ORDER BY detected_at DESC
            "#,
        )
        .bind(pair)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(parse_opportunity_row).collect()
    }

    async fn count(&self) -> Result<i64, StorageError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM opportunities")
            .fetch_one(&self.pool)
            .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count)
    }

    async fn close(&self) -> Result<(), StorageError> {
        self.pool.close().await;
        Ok(())
    }
}

/// Parses an opportunity from a database row.
fn parse_opportunity_row(row: &sqlx::sqlite::SqliteRow) -> Result<Opportunity, StorageError> {
    let opp_type_str: String = row.try_get("type")?;
    let opportunity_type = OpportunityType::from_str(&opp_type_str)
        .map_err(|e| StorageError::InvalidData(e.to_string()))?;

    let buy_price_str: String = row.try_get("buy_price")?;
    let buy_price = Decimal::from_str(&buy_price_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid buy_price: {}", e)))?;

    let sell_price_str: String = row.try_get("sell_price")?;
    let sell_price = Decimal::from_str(&sell_price_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid sell_price: {}", e)))?;

    let quantity_str: String = row.try_get("quantity")?;
    let quantity = Decimal::from_str(&quantity_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid quantity: {}", e)))?;

    let gross_profit_str: String = row.try_get("gross_profit")?;
    let gross_profit = Decimal::from_str(&gross_profit_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid gross_profit: {}", e)))?;

    let net_profit_str: String = row.try_get("net_profit")?;
    let net_profit = Decimal::from_str(&net_profit_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid net_profit: {}", e)))?;

    let profit_percent_str: String = row.try_get("profit_percent")?;
    let profit_percent = Decimal::from_str(&profit_percent_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid profit_percent: {}", e)))?;

    let buy_fee_str: String = row.try_get("buy_fee")?;
    let buy_fee = Decimal::from_str(&buy_fee_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid buy_fee: {}", e)))?;

    let sell_fee_str: String = row.try_get("sell_fee")?;
    let sell_fee = Decimal::from_str(&sell_fee_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid sell_fee: {}", e)))?;

    let detected_at_str: String = row.try_get("detected_at")?;
    let detected_at = DateTime::parse_from_rfc3339(&detected_at_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid detected_at: {}", e)))?
        .with_timezone(&Utc);

    let expires_at_str: String = row.try_get("expires_at")?;
    let expires_at = DateTime::parse_from_rfc3339(&expires_at_str)
        .map_err(|e| StorageError::InvalidData(format!("Invalid expires_at: {}", e)))?
        .with_timezone(&Utc);

    Ok(Opportunity {
        id: row.try_get("id")?,
        opportunity_type,
        pair: row.try_get("pair")?,
        buy_exchange: row.try_get("buy_exchange")?,
        sell_exchange: row.try_get("sell_exchange")?,
        buy_price,
        sell_price,
        quantity,
        gross_profit,
        net_profit,
        profit_percent,
        buy_fee,
        sell_fee,
        detected_at,
        expires_at,
    })
}
