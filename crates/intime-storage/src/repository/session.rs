//! Session repository — open/close category+context merges (no action spans).

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    db::models::{ActivityRuleRecord, CategoryRecord, SessionRecord},
    error::StorageError,
};

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn open_session(
        &self,
        started_at: DateTime<Utc>,
        intent: Option<&str>,
        title: Option<&str>,
        source: &str,
        category_id: Option<i64>,
        context_key: Option<&str>,
        app_id: Option<i64>,
    ) -> Result<i64, StorageError>;

    async fn close_session(
        &self,
        session_id: i64,
        ended_at: DateTime<Utc>,
        summary: Option<&str>,
    ) -> Result<(), StorageError>;

    async fn get_session(&self, id: i64) -> Result<SessionRecord, StorageError>;

    async fn list_categories(&self) -> Result<Vec<CategoryRecord>, StorageError>;
    async fn list_activity_rules(&self) -> Result<Vec<ActivityRuleRecord>, StorageError>;

    /// User or LLM adjustment — insert or update by (field, op, pattern).
    async fn upsert_activity_rule(
        &self,
        category_id: i64,
        match_field: &str,
        match_op: &str,
        pattern: &str,
        priority: i64,
        source: &str,
        notes: Option<&str>,
    ) -> Result<i64, StorageError>;
}
