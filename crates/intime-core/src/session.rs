//! Higher-level timeline sessions (context + category merges).
//!
//! Discrete UI/media verbs are raw `EventData::UiAction` rows — not session children.
//! Category assignment comes from `activity_rule` (seed / user / future local LLM).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Heuristic,
    Llm,
    Manual,
}

impl SessionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Heuristic => "heuristic",
            Self::Llm => "llm",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Denormalized category slug for convenience (prefer `category_id` join).
    pub intent: Option<String>,
    pub intent_confidence: Option<f64>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub source: String,
    pub category_id: Option<i64>,
    pub context_key: Option<String>,
    pub app_id: Option<i64>,
}

/// Thresholds for promoting buffered activity into a real session row.
#[derive(Debug, Clone)]
pub struct SessionPromotionPolicy {
    /// Minimum wall time in the same context before opening a session.
    pub min_duration_secs: u64,
    /// Minimum meaningful events in the same context before opening.
    pub min_meaningful_events: u32,
}

impl Default for SessionPromotionPolicy {
    fn default() -> Self {
        Self {
            min_duration_secs: 20,
            min_meaningful_events: 3,
        }
    }
}
