//! Session repository — open/close category+context merges (no action spans).

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::{
    db::models::{ActivityRuleRecord, CategoryRecord, SessionRecord},
    error::StorageError,
};

/// Filters for listing sessions (AI / UI / MCP query surface).
#[derive(Debug, Clone, Default)]
pub struct SessionListFilter {
    pub since: Option<DateTime<Utc>>,
    pub until: Option<DateTime<Utc>>,
    /// Match `session.intent` (category slug).
    pub intent: Option<String>,
    /// `context_key LIKE '{prefix}%'` — e.g. `repo:github.com:foo/bar` or `ws:intime-rs`.
    pub context_key_prefix: Option<String>,
    pub important_only: bool,
    pub open_only: bool,
    pub limit: i64,
}

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
        ended_reason: Option<&str>,
    ) -> Result<(), StorageError>;

    /// Most recent session for this context (open or closed), if any.
    async fn find_session_by_context(
        &self,
        context_key: &str,
        category_id: Option<i64>,
    ) -> Result<Option<SessionRecord>, StorageError>;

    /// Clear `ended_at` so the session continues (same video / same context).
    async fn reopen_session(&self, session_id: i64) -> Result<(), StorageError>;

    /// Refine identity of an open session (media URL id, better title, category upgrade).
    async fn refine_session(
        &self,
        session_id: i64,
        intent: Option<&str>,
        title: Option<&str>,
        category_id: Option<i64>,
        context_key: Option<&str>,
    ) -> Result<(), StorageError>;

    /// Close every open session except `keep_id` (recovers orphans after restart / lost tracker state).
    async fn close_orphaned_open_sessions(
        &self,
        ended_at: DateTime<Utc>,
        keep_id: Option<i64>,
        ended_reason: &str,
    ) -> Result<u64, StorageError>;

    /// Backfill `app_id` when media started via MPRIS before the browser AppSeen.
    async fn set_session_app_id(&self, session_id: i64, app_id: i64) -> Result<(), StorageError>;

    /// Pin a session so screenshot retention never compact/deletes its images.
    async fn set_session_important(
        &self,
        session_id: i64,
        important: bool,
    ) -> Result<(), StorageError>;

    async fn get_session(&self, id: i64) -> Result<SessionRecord, StorageError>;

    /// Newest-first sessions matching optional filters.
    async fn list_sessions(
        &self,
        filter: SessionListFilter,
    ) -> Result<Vec<SessionRecord>, StorageError>;

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
