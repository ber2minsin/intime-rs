use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    db::{
        SqliteRepository,
        models::{ActivityRuleRecord, CategoryRecord, SessionRecord},
    },
    error::StorageError,
    repository::session::{SessionListFilter, SessionRepository},
};

#[async_trait]
impl SessionRepository for SqliteRepository {
    async fn open_session(
        &self,
        started_at: DateTime<Utc>,
        intent: Option<&str>,
        title: Option<&str>,
        source: &str,
        category_id: Option<i64>,
        context_key: Option<&str>,
        app_id: Option<i64>,
    ) -> Result<i64, StorageError> {
        let id = sqlx::query_scalar(
            "INSERT INTO session(started_at, intent, title, source, category_id, context_key, app_id)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(started_at)
        .bind(intent)
        .bind(title)
        .bind(source)
        .bind(category_id)
        .bind(context_key)
        .bind(app_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    async fn close_session(
        &self,
        session_id: i64,
        ended_at: DateTime<Utc>,
        summary: Option<&str>,
        ended_reason: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE session SET ended_at = ?, summary = COALESCE(?, summary),
             ended_reason = COALESCE(?, ended_reason) WHERE id = ?",
        )
        .bind(ended_at)
        .bind(summary)
        .bind(ended_reason)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn find_session_by_context(
        &self,
        context_key: &str,
        category_id: Option<i64>,
    ) -> Result<Option<SessionRecord>, StorageError> {
        let record = sqlx::query_as::<_, SessionRecord>(
            "SELECT id, started_at, ended_at, intent, intent_confidence, title, summary, source,
                    category_id, context_key, app_id, created_at, important, ended_reason
             FROM session
             WHERE context_key = ?
               AND (? IS NULL OR category_id = ?)
             ORDER BY COALESCE(ended_at, started_at) DESC, id DESC
             LIMIT 1",
        )
        .bind(context_key)
        .bind(category_id)
        .bind(category_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(record)
    }

    async fn reopen_session(&self, session_id: i64) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE session SET ended_at = NULL, ended_reason = NULL, summary = NULL WHERE id = ?",
        )
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn refine_session(
        &self,
        session_id: i64,
        intent: Option<&str>,
        title: Option<&str>,
        category_id: Option<i64>,
        context_key: Option<&str>,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE session SET
               intent = COALESCE(?, intent),
               title = COALESCE(?, title),
               category_id = COALESCE(?, category_id),
               context_key = COALESCE(?, context_key)
             WHERE id = ? AND ended_at IS NULL",
        )
        .bind(intent)
        .bind(title)
        .bind(category_id)
        .bind(context_key)
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn close_orphaned_open_sessions(
        &self,
        ended_at: DateTime<Utc>,
        keep_id: Option<i64>,
        ended_reason: &str,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "UPDATE session SET ended_at = ?, ended_reason = COALESCE(ended_reason, ?),
             summary = COALESCE(summary, 'orphan recovered')
             WHERE ended_at IS NULL AND (? IS NULL OR id != ?)",
        )
        .bind(ended_at)
        .bind(ended_reason)
        .bind(keep_id)
        .bind(keep_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn set_session_app_id(&self, session_id: i64, app_id: i64) -> Result<(), StorageError> {
        sqlx::query("UPDATE session SET app_id = ? WHERE id = ? AND app_id IS NULL")
            .bind(app_id)
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn set_session_important(
        &self,
        session_id: i64,
        important: bool,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE session SET important = ? WHERE id = ?")
            .bind(if important { 1_i64 } else { 0_i64 })
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_session(&self, id: i64) -> Result<SessionRecord, StorageError> {
        let record = sqlx::query_as::<_, SessionRecord>(
            "SELECT id, started_at, ended_at, intent, intent_confidence, title, summary, source,
                    category_id, context_key, app_id, created_at, important, ended_reason
             FROM session WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(record)
    }

    async fn list_sessions(
        &self,
        filter: SessionListFilter,
    ) -> Result<Vec<SessionRecord>, StorageError> {
        let limit = if filter.limit <= 0 { 100 } else { filter.limit };
        let prefix = filter
            .context_key_prefix
            .as_ref()
            .map(|p| format!("{p}%"));
        let records = sqlx::query_as::<_, SessionRecord>(
            "SELECT id, started_at, ended_at, intent, intent_confidence, title, summary, source,
                    category_id, context_key, app_id, created_at, important, ended_reason
             FROM session
             WHERE (?1 IS NULL OR started_at >= ?1)
               AND (?2 IS NULL OR started_at <= ?2)
               AND (?3 IS NULL OR intent = ?3)
               AND (?4 IS NULL OR context_key LIKE ?4)
               AND (?5 = 0 OR important = 1)
               AND (?6 = 0 OR ended_at IS NULL)
             ORDER BY started_at DESC, id DESC
             LIMIT ?7",
        )
        .bind(filter.since)
        .bind(filter.until)
        .bind(filter.intent.as_deref())
        .bind(prefix.as_deref())
        .bind(if filter.important_only { 1_i64 } else { 0_i64 })
        .bind(if filter.open_only { 1_i64 } else { 0_i64 })
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(records)
    }

    async fn list_categories(&self) -> Result<Vec<CategoryRecord>, StorageError> {
        let rows = sqlx::query_as::<_, CategoryRecord>(
            "SELECT id, slug, name, description, occupation_tags, source, created_at FROM category ORDER BY slug",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn list_activity_rules(&self) -> Result<Vec<ActivityRuleRecord>, StorageError> {
        let rows = sqlx::query_as::<_, ActivityRuleRecord>(
            "SELECT id, category_id, match_field, match_op, pattern, priority, enabled, source, notes, created_at
             FROM activity_rule ORDER BY priority DESC, id ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn upsert_activity_rule(
        &self,
        category_id: i64,
        match_field: &str,
        match_op: &str,
        pattern: &str,
        priority: i64,
        source: &str,
        notes: Option<&str>,
    ) -> Result<i64, StorageError> {
        let id = sqlx::query_scalar(
            "INSERT INTO activity_rule(category_id, match_field, match_op, pattern, priority, enabled, source, notes)
             VALUES (?, ?, ?, ?, ?, 1, ?, ?)
             ON CONFLICT(match_field, match_op, pattern) DO UPDATE SET
               category_id = excluded.category_id,
               priority = excluded.priority,
               enabled = 1,
               source = excluded.source,
               notes = excluded.notes
             RETURNING id",
        )
        .bind(category_id)
        .bind(match_field)
        .bind(match_op)
        .bind(pattern)
        .bind(priority)
        .bind(source)
        .bind(notes)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }
}
