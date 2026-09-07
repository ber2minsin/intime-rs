//! Runtime feature flags for privacy-sensitive collection and AI features.
//!
//! All flags default to enabling the existing behavior except LLM session
//! inference (off until a model pipeline exists). Users can opt out via env.

use serde::{Deserialize, Serialize};

/// Collection and AI opt-in/out switches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Capture screenshots on focus/title changes.
    pub screenshots_enabled: bool,
    /// Queue embedding jobs for captured screenshots.
    pub embeddings_enabled: bool,
    /// Persist focused UI element / automation metadata (more intrusive).
    pub rich_ui_metadata: bool,
    /// Persist document paths / names / URLs (from a11y or literal title parse).
    pub document_context: bool,
    /// Group events into heuristic category sessions (not every flicker).
    pub session_grouping: bool,
    /// Allow offline/on-demand LLM to suggest categories/rules and refine sessions.
    pub session_llm: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            screenshots_enabled: true,
            embeddings_enabled: true,
            rich_ui_metadata: true,
            document_context: true,
            session_grouping: true,
            session_llm: false,
        }
    }
}

impl FeatureFlags {
    /// Load from environment variables (`INTIME_*`), falling back to defaults.
    pub fn from_env() -> Self {
        let mut flags = Self::default();
        flags.screenshots_enabled = env_bool("INTIME_SCREENSHOTS_ENABLED", flags.screenshots_enabled);
        flags.embeddings_enabled = env_bool("INTIME_EMBEDDINGS_ENABLED", flags.embeddings_enabled);
        flags.rich_ui_metadata = env_bool("INTIME_RICH_UI_METADATA", flags.rich_ui_metadata);
        flags.document_context = env_bool("INTIME_DOCUMENT_CONTEXT", flags.document_context);
        flags.session_grouping = env_bool("INTIME_SESSION_GROUPING", flags.session_grouping);
        flags.session_llm = env_bool("INTIME_SESSION_LLM", flags.session_llm);
        flags
    }

    /// Strip fields the user opted out of before persistence / downstream AI.
    pub fn sanitize_metadata(&self, meta: &mut crate::models::EventMetadata) {
        if !self.rich_ui_metadata {
            meta.focused_element = None;
            meta.focused_element_class = None;
            meta.focused_control_type = None;
            meta.automation_id = None;
            meta.text_changed = false;
        }
        if !self.document_context {
            meta.document_path = None;
            meta.document_name = None;
            meta.url = None;
            meta.workspace_path = None;
        }
    }
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::EventMetadata;

    #[test]
    fn sanitize_honors_opt_outs() {
        let flags = FeatureFlags {
            rich_ui_metadata: false,
            document_context: false,
            ..Default::default()
        };
        let mut meta = EventMetadata {
            focused_element: Some("Body".into()),
            document_path: Some("/tmp/a.rs".into()),
            url: Some("https://example.com".into()),
            text_changed: true,
            ..Default::default()
        };
        flags.sanitize_metadata(&mut meta);
        assert!(meta.focused_element.is_none());
        assert!(meta.document_path.is_none());
        assert!(meta.url.is_none());
        assert!(!meta.text_changed);
    }
}
