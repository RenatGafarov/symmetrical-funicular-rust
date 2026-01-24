//! Storage interfaces and implementations for persisting arbitrage data.

mod sqlite;

pub use sqlite::{SqliteStorage, SqliteStorageConfig};

use crate::domain::Opportunity;
use async_trait::async_trait;

/// OpportunityStorage defines the interface for storing arbitrage opportunities.
#[async_trait]
pub trait OpportunityStorage: Send + Sync {
    /// Save persists an opportunity to storage.
    /// Returns true if the opportunity was saved (new), false if it already exists.
    async fn save(&self, opp: &Opportunity) -> Result<bool, StorageError>;

    /// GetByID retrieves an opportunity by its ID.
    async fn get_by_id(&self, id: &str) -> Result<Option<Opportunity>, StorageError>;

    /// GetAll retrieves all stored opportunities.
    async fn get_all(&self) -> Result<Vec<Opportunity>, StorageError>;

    /// GetByPair retrieves opportunities for a specific trading pair.
    async fn get_by_pair(&self, pair: &str) -> Result<Vec<Opportunity>, StorageError>;

    /// Count returns the total number of stored opportunities.
    async fn count(&self) -> Result<i64, StorageError>;

    /// Close closes the storage connection.
    async fn close(&self) -> Result<(), StorageError>;
}

/// StorageError represents errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
