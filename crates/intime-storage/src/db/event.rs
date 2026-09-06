use async_trait::async_trait;
use intime_core::models::Event;

use crate::{db::SqliteRepository, error::StorageError, repository::event::EventRepository};

#[async_trait]
impl EventRepository for SqliteRepository {
    async fn add_event(
        &self,
        event: &Event,
        app_id: Option<i64>,
        screenshot_path: &Option<String>,
    ) -> Result<i64, StorageError> {
        let event_name = event.data.name();
        let occured_at = event.timestamp.as_datetime();
        let payload = Some(serde_json::to_string(event).map_err(StorageError::Serialization)?);

        let event_id = sqlx::query_scalar!(
            "INSERT INTO event(event_type, occured_at, payload, app_id, screenshot_path) VALUES (?, ?, ?, ?, ?) RETURNING id",
            event_name,
            occured_at,
            payload,
            app_id,
            screenshot_path
        ).fetch_one(&self.pool).await?;
        Ok(event_id)
    }
}
