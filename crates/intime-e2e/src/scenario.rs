use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};

use serde::Deserialize;

/// Declarative timeline scenario loaded from JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub name: String,
    pub description: Option<String>,
    pub steps: Vec<ScenarioStep>,
    pub expect: ScenarioExpect,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScenarioStep {
    AppSeen {
        handle: u64,
        title: String,
        file_path: String,
        #[serde(default)]
        company: Option<String>,
        #[serde(default)]
        product: Option<String>,
        #[serde(default)]
        fixture: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        workspace: Option<String>,
    },
    WindowFocus {
        handle: u64,
        title: String,
        #[serde(default)]
        focused_element: Option<String>,
        #[serde(default)]
        fixture: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        workspace: Option<String>,
        #[serde(default)]
        document_name: Option<String>,
    },
    TitleChange {
        handle: u64,
        new_title: String,
        #[serde(default)]
        fixture: Option<String>,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        workspace: Option<String>,
    },
    IdleStart,
    IdleEnd,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScenarioExpect {
    pub min_events: usize,
    #[serde(default)]
    pub event_types: Vec<String>,
    #[serde(default)]
    pub min_screenshots: usize,
    #[serde(default)]
    pub min_embeddings: usize,
    #[serde(default)]
    pub company: Option<String>,
    /// Minimum number of session rows created during the scenario.
    #[serde(default)]
    pub min_sessions: Option<usize>,
    /// Each listed intent (category slug) must appear on at least one session.
    #[serde(default)]
    pub intents: Vec<String>,
    /// Each prefix must match at least one session `context_key`.
    #[serde(default)]
    pub context_key_prefixes: Vec<String>,
}

impl Scenario {
    pub fn load(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Build per-handle fixture queues in step order (relative to scenario dir).
    pub fn fixture_queues(&self) -> HashMap<u64, VecDeque<PathBuf>> {
        let mut queues: HashMap<u64, VecDeque<PathBuf>> = HashMap::new();
        for step in &self.steps {
            let (handle, fixture) = match step {
                ScenarioStep::AppSeen { handle, fixture, .. }
                | ScenarioStep::WindowFocus { handle, fixture, .. } => (*handle, fixture.as_ref()),
                ScenarioStep::TitleChange {
                    handle, fixture, ..
                } => (*handle, fixture.as_ref()),
                _ => continue,
            };
            if let Some(name) = fixture {
                queues
                    .entry(handle)
                    .or_default()
                    .push_back(PathBuf::from("fixtures").join(name));
            }
        }
        queues
    }
}
