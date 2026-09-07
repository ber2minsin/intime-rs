use async_trait::async_trait;
use intime_core::models::Event;

use crate::{
    db::{SqliteRepository, models::EventRecord},
    error::StorageError,
    repository::event::{EventRepository, ScreenshotCandidate},
};

#[async_trait]
impl EventRepository for SqliteRepository {
    async fn add_event(
        &self,
        event: &Event,
        app_id: Option<i64>,
        screenshot_path: &Option<String>,
        session_id: Option<i64>,
    ) -> Result<i64, StorageError> {
        let event_name = event.data.name();
        let occured_at = event.timestamp.as_datetime();
        let payload = Some(serde_json::to_string(event).map_err(StorageError::Serialization)?);
        let window_handle = event.data.window_handle().map(|h| h as i64);
        let process_id = event.metadata.process_id.map(|p| p as i64);
        let text_changed = if event.metadata.text_changed {
            1_i64
        } else {
            0_i64
        };
        let screenshot_tier = screenshot_path.as_ref().map(|_| "full".to_string());

        let event_id = sqlx::query_scalar(
            r#"INSERT INTO event(
                event_type, occured_at, payload, app_id, screenshot_path,
                window_handle, window_title, process_id, executable_path,
                document_path, document_name, workspace_path, text_changed,
                session_id, screenshot_tier
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING id"#,
        )
        .bind(event_name)
        .bind(occured_at)
        .bind(payload)
        .bind(app_id)
        .bind(screenshot_path)
        .bind(window_handle)
        .bind(&event.metadata.window_title)
        .bind(process_id)
        .bind(&event.metadata.executable_path)
        .bind(&event.metadata.document_path)
        .bind(&event.metadata.document_name)
        .bind(&event.metadata.workspace_path)
        .bind(text_changed)
        .bind(session_id)
        .bind(screenshot_tier)
        .fetch_one(&self.pool)
        .await?;
        Ok(event_id)
    }

    async fn get_event(&self, id: i64) -> Result<EventRecord, StorageError> {
        let record = sqlx::query_as::<_, EventRecord>(EVENT_SELECT)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(record)
    }

    async fn list_events(&self, limit: i64) -> Result<Vec<EventRecord>, StorageError> {
        let records = sqlx::query_as::<_, EventRecord>(&format!(
            "{EVENT_SELECT_NO_WHERE} ORDER BY id ASC LIMIT ?"
        ))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(records)
    }

    async fn list_screenshot_candidates(
        &self,
        limit: i64,
    ) -> Result<Vec<ScreenshotCandidate>, StorageError> {
        let rows = sqlx::query_as::<_, ScreenshotCandidate>(
            r#"SELECT
                e.id AS event_id,
                e.screenshot_path AS path,
                e.screenshot_tier AS tier,
                e.occured_at AS occured_at,
                COALESCE(s.important, 0) AS session_important
             FROM event e
             LEFT JOIN session s ON s.id = e.session_id
             WHERE e.screenshot_path IS NOT NULL
             ORDER BY e.occured_at ASC
             LIMIT ?"#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn clear_screenshot(&self, event_id: i64) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE event SET screenshot_path = NULL, screenshot_tier = NULL WHERE id = ?",
        )
        .bind(event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn set_screenshot_tier(
        &self,
        event_id: i64,
        tier: &str,
        path: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE event SET screenshot_tier = ?, screenshot_path = COALESCE(?, screenshot_path) WHERE id = ?",
        )
        .bind(tier)
        .bind(path)
        .bind(event_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

const EVENT_SELECT_NO_WHERE: &str = r#"SELECT
    id,
    app_id,
    event_type,
    occured_at,
    created_at,
    payload,
    screenshot_path,
    window_handle,
    window_title,
    process_id,
    executable_path,
    document_path,
    document_name,
    workspace_path,
    text_changed,
    session_id,
    screenshot_tier
FROM event"#;

const EVENT_SELECT: &str = r#"SELECT
    id,
    app_id,
    event_type,
    occured_at,
    created_at,
    payload,
    screenshot_path,
    window_handle,
    window_title,
    process_id,
    executable_path,
    document_path,
    document_name,
    workspace_path,
    text_changed,
    session_id,
    screenshot_tier
FROM event
WHERE id = ?"#;
