use async_trait::async_trait;
use chrono::{DateTime, Utc};
use intime_core::models::Event;

use crate::{db::models::EventRecord, error::StorageError};

/// Event row that still has an on-disk screenshot (for retention sweeps).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ScreenshotCandidate {
    pub event_id: i64,
    pub path: String,
    pub tier: Option<String>,
    pub occured_at: DateTime<Utc>,
    pub session_important: i64,
}

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

    /// Oldest-first events that still reference a screenshot file.
    async fn list_screenshot_candidates(
        &self,
        limit: i64,
    ) -> Result<Vec<ScreenshotCandidate>, StorageError>;

    async fn clear_screenshot(&self, event_id: i64) -> Result<(), StorageError>;

    async fn set_screenshot_tier(
        &self,
        event_id: i64,
        tier: &str,
        path: Option<&str>,
    ) -> Result<(), StorageError>;
}
