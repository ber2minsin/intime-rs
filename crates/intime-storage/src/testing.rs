//! Helpers for spinning up an isolated SQLite database with migrations and sqlite-vec.

use std::path::PathBuf;

use sqlx::SqlitePool;
use tempfile::TempDir;

use crate::{db::pool::build_pool, error::StorageError, storage::Storage};

/// Owned test database. Keep this value alive for the duration of the test so
/// the temporary directory (and therefore the sqlite file) is not deleted.
pub struct TestDatabase {
    pub storage: Storage,
    pub pool: SqlitePool,
    pub database_url: String,
    _dir: TempDir,
}

impl TestDatabase {
    /// Create a fresh on-disk sqlite database in a temporary directory, register
    /// sqlite-vec, and run all migrations.
    pub async fn new() -> Result<Self, StorageError> {
        let dir = TempDir::new()
            .map_err(|e| StorageError::Other(anyhow::anyhow!("failed to create temp dir: {e}")))?;
        let path: PathBuf = dir.path().join("intime-test.db");
        let database_url = format!("sqlite://{}?mode=rwc", path.display());

        let pool = build_pool(&database_url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;

        Ok(Self {
            storage: Storage::from_pool(pool.clone()),
            pool,
            database_url,
            _dir: dir,
        })
    }
}
