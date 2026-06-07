use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use crate::error::DbError;

/// A thin wrapper around `SqlitePool` to carry Zircon-specific config.
pub struct DbPool {
    inner: SqlitePool,
}

impl DbPool {
    /// Open (or create) an SQLite database at `url`, e.g. `sqlite://zircon.db`.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect(url)
            .await?;
        Ok(DbPool { inner: pool })
    }

    /// Expose the underlying pool for direct `sqlx` queries.
    pub fn inner(&self) -> &SqlitePool {
        &self.inner
    }
}
