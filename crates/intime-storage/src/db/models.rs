use sqlx::types::chrono::{DateTime, Utc};

#[derive(sqlx::FromRow)]
pub struct CompanyRecord {
    pub id: i64,
    pub company_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
pub struct EventRecord {
    pub id: i64,
    pub app_id: Option<i64>,
    pub event_type: String, // TODO actual event types
    pub occured_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub payload: Option<String>,
    pub screenshot_path: Option<String>,
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
