use async_trait::async_trait;
use intime_core::models::Event;

use crate::{db::models::EventRecord, error::StorageError};

#[async_trait]
pub trait EventRepository: Send + Sync {
    async fn add_event(
        &self,
        event: &Event,
        app_id: Option<i64>,
        screenshot_path: &Option<String>,
        session_id: Option<i64>,
    ) -> Result<i64, StorageError>;

    async fn get_event(&self, id: i64) -> Result<EventRecord, StorageError>;

    async fn list_events(&self, limit: i64) -> Result<Vec<EventRecord>, StorageError>;
}
