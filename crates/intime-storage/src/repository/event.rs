use async_trait::async_trait;
use intime_core::models::Event;

use crate::error::StorageError;

#[async_trait]
pub trait EventRepository: Send + Sync {
    // TODO add actual event type enum
    async fn add_event(
        &self,
        event: &Event,
        app_id: Option<i64>,
        screenshot_path: &Option<String>,
    ) -> Result<i64, StorageError>;
}
