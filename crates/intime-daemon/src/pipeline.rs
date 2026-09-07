use std::sync::Arc;

use anyhow::{Context, Result};
use intime_ai::{
    embedding::EmbeddingServer,
    models::{EmbeddingRequest, EmbeddingTask},
};
use intime_core::{
    category::{match_category, ActivityRule, CategoryMatchInput},
    context::enrich_from_window_title,
    features::FeatureFlags,
    models::{Event, EventData},
};
use intime_storage::storage::Storage;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::{
    orchestrator::ScreenshotOrchestrator,
    session_tracker::{match_input_from_event, SessionTracker},
};

/// Cached rules for matching — refresh when LLM/user upserts rules later.
pub struct CategoryRulesCache {
    rules: Vec<ActivityRule>,
    slug_by_id: std::collections::HashMap<i64, String>,
}

impl CategoryRulesCache {
    pub async fn load(storage: &Storage) -> Result<Self> {
        let cats = storage.session_repository.list_categories().await?;
        let slug_by_id = cats.into_iter().map(|c| (c.id, c.slug)).collect();
        let rows = storage.session_repository.list_activity_rules().await?;
        let rules = rows
            .into_iter()
            .map(|r| ActivityRule {
                id: r.id,
                category_id: r.category_id,
                match_field: r.match_field,
                match_op: r.match_op,
                pattern: r.pattern,
                priority: r.priority,
                enabled: r.enabled != 0,
                source: r.source,
                notes: r.notes,
            })
            .collect();
        Ok(Self { rules, slug_by_id })
    }

    pub fn slug(&self, category_id: i64) -> &str {
        self.slug_by_id
            .get(&category_id)
            .map(|s| s.as_str())
            .unwrap_or("unknown")
    }

    pub fn match_input(
        &self,
        input: &CategoryMatchInput,
    ) -> Option<(intime_core::category::CategoryHit, &str)> {
        let hit = match_category(&self.rules, input)?;
        let slug = self.slug(hit.category_id);
        Some((hit, slug))
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }
}

/// Persist an event, optionally capture a screenshot, and queue embedding work.
pub async fn handle_incoming_event(
    event: Arc<Event>,
    storage: &Storage,
    screenshot_orchestrator: &mut ScreenshotOrchestrator,
    embedding_queue: mpsc::Sender<EmbeddingTask>,
    flags: &FeatureFlags,
    sessions: &mut SessionTracker,
    rules: &CategoryRulesCache,
) -> Result<()> {
    let mut event = (*event).clone();

    let product_hint = match &event.data {
        EventData::AppSeen { details, .. } => details.product_name.clone(),
        _ => None,
    };

    if flags.document_context {
        enrich_from_window_title(&mut event.metadata, product_hint.as_deref());
    }
    flags.sanitize_metadata(&mut event.metadata);

    let (app_id, aumid, company, display_name) = match &event.data {
        EventData::AppSeen {
            fingerprint,
            details,
            ..
        } => {
            let id = storage
                .app_repository
                .ensure_app(*fingerprint, details)
                .await
                .context("failed to ensure app identity rows")?;
            info!("Ensured app identity: {}", details.display_name());
            (
                Some(id),
                details.aumid.clone(),
                details.company(),
                Some(details.display_name()),
            )
        }
        _ => {
            let id = if let Some(fp) = event.data.fingerprint() {
                storage.app_repository.get_app_id(fp).await.ok()
            } else {
                None
            };
            (id, None, None, None)
        }
    };

    if event.metadata.executable_path.is_none() {
        if let EventData::AppSeen { details, .. } = &event.data {
            if !details.file_path.is_empty() {
                event.metadata.executable_path = Some(details.file_path.clone());
            }
        }
    }

    let input = match_input_from_event(
        &event,
        aumid,
        product_hint
            .clone()
            .or_else(|| display_name.clone()),
        display_name,
        company,
    );
    let matched = rules.match_input(&input);
    let (hit, slug) = match &matched {
        Some((h, s)) => (Some(h), *s),
        None => (None, "unknown"),
    };

    let session_id = sessions
        .observe(storage, &event, app_id, hit, slug, product_hint.as_deref())
        .await
        .context("session tracker")?;

    let image_path = if flags.screenshots_enabled {
        match screenshot_orchestrator.process_event(&event) {
            Ok(path) => path.map(|path| path.to_string_lossy().into_owned()),
            Err(e) => {
                warn!("Screenshot skipped: {e:#}");
                None
            }
        }
    } else {
        None
    };

    let event_id = storage
        .event_repository
        .add_event(&event, app_id, &image_path, session_id)
        .await
        .context("failed to persist event")?;

    if flags.embeddings_enabled {
        if let Some(path) = image_path {
            let task = EmbeddingTask {
                event_id,
                req: EmbeddingRequest::Image { image_path: path },
            };
            embedding_queue
                .send(task)
                .await
                .context("embedding queue closed")?;
        }
    }

    Ok(())
}

pub async fn start_embedding_worker(
    rx: &mut mpsc::Receiver<EmbeddingTask>,
    service: &mut (dyn EmbeddingServer + Send),
    storage: &Storage,
) -> Result<()> {
    while let Some(task) = rx.recv().await {
        match service.make_request(task.req).await {
            Ok(resp) => {
                storage
                    .embedding_repository
                    .add_embedding(task.event_id, resp)
                    .await
                    .context("failed to store embedding")?;
            }
            Err(e) => {
                warn!("Embedding request failed (continuing): {e:#}");
            }
        }
    }

    Ok(())
}
