use std::sync::Arc;

use crate::{
    config::StorageConfig,
    db::{SqliteRepository, pool::build_pool},
    error::StorageError,
    repository::{app::AppRepository, embedding::EmbeddingRepository, event::EventRepository},
};
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct Storage {
    pub app_repository: Arc<dyn AppRepository>,
    pub event_repository: Arc<dyn EventRepository>,
    pub embedding_repository: Arc<dyn EmbeddingRepository>,
}

impl Storage {
    pub async fn connect(config: &StorageConfig) -> Result<Self, StorageError> {
        let pool = build_pool(&config.database_file).await?;
        Ok(Self::from_pool(pool))
    }

    pub fn from_pool(pool: SqlitePool) -> Self {
        let app_repo = SqliteRepository::new(pool.clone());
        let event_repo = SqliteRepository::new(pool.clone());
        let embedding_repo = SqliteRepository::new(pool);

        Storage {
            app_repository: Arc::new(app_repo),
            event_repository: Arc::new(event_repo),
            embedding_repository: Arc::new(embedding_repo),
        }
    }
}
