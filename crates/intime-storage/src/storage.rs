use std::sync::Arc;

use crate::{
    config::StorageConfig,
    db::{SqliteRepository, pool::build_pool},
    error::StorageError,
    repository::{
        app::AppRepository, embedding::EmbeddingRepository, event::EventRepository,
        session::SessionRepository,
    },
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Storage {
    pub app_repository: Arc<dyn AppRepository>,
    pub event_repository: Arc<dyn EventRepository>,
    pub embedding_repository: Arc<dyn EmbeddingRepository>,
    pub session_repository: Arc<dyn SessionRepository>,
}

impl Storage {
    pub async fn connect(config: &StorageConfig) -> Result<Self, StorageError> {
        let pool = build_pool(&config.database_file).await?;
        // Keep schema in sync so session promote (ended_reason, …) never fails
        // because the daemon was rebuilt ahead of a manual migrate.
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self::from_pool(pool))
    }

    pub fn from_pool(pool: SqlitePool) -> Self {
        let repo = SqliteRepository::new(pool);
        Storage {
            app_repository: Arc::new(repo.clone()),
            event_repository: Arc::new(repo.clone()),
            embedding_repository: Arc::new(repo.clone()),
            session_repository: Arc::new(repo),
        }
    }
}
