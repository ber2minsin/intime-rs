use sqlx::types::chrono::{DateTime, Utc};

#[derive(Debug, sqlx::FromRow)]
pub struct CompanyRecord {
    pub id: i64,
    pub company_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AumidRecord {
    pub id: i64,
    pub aumid: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct EventRecord {
    pub id: i64,
    pub app_id: Option<i64>,
    pub event_type: String,
    pub occured_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub payload: Option<String>,
    pub screenshot_path: Option<String>,
    pub window_handle: Option<i64>,
    pub window_title: Option<String>,
    pub process_id: Option<i64>,
    pub executable_path: Option<String>,
    pub document_path: Option<String>,
    pub document_name: Option<String>,
    pub workspace_path: Option<String>,
    pub text_changed: i64,
    pub session_id: Option<i64>,
    pub screenshot_tier: Option<String>,
}

#[derive(sqlx::FromRow, Debug)]
pub struct EmbeddingRecord {
    pub id: i64,
    pub event_id: i64,
    pub embedding_type: String,
    pub embedding_backend: String,
    pub embedding_data: Vec<u8>,
    pub similarity: f64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SessionRecord {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub intent: Option<String>,
    pub intent_confidence: Option<f64>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub source: String,
    pub category_id: Option<i64>,
    pub context_key: Option<String>,
    pub app_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub important: i64,
    pub ended_reason: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CategoryRecord {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub occupation_tags: String,
    pub source: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ActivityRuleRecord {
    pub id: i64,
    pub category_id: i64,
    pub match_field: String,
    pub match_op: String,
    pub pattern: String,
    pub priority: i64,
    pub enabled: i64,
    pub source: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}
