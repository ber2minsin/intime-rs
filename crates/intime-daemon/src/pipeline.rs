use std::sync::Arc;

use anyhow::{Context, Result};
use intime_ai::{
    embedding::EmbeddingServer,
    models::{EmbeddingRequest, EmbeddingTask},
};
use intime_core::models::{Event, EventData};
use intime_storage::storage::Storage;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::orchestrator::ScreenshotOrchestrator;

/// Persist an event, optionally capture a screenshot, and queue embedding work.
pub async fn handle_incoming_event(
    event: Arc<Event>,
    storage: &Storage,
    screenshot_orchestrator: &mut ScreenshotOrchestrator,
    embedding_queue: mpsc::Sender<EmbeddingTask>,
) -> Result<()> {
    if let EventData::AppSeen {
        fingerprint,
        details,
        ..
    } = &event.data
    {
        let mut company_id = None;
        if let Some(name) = details.company() {
            company_id = match storage.app_repository.seen_company(&name).await {
                Ok(id) => Some(id),
                Err(_) => Some(
                    storage
                        .app_repository
                        .add_company(&name)
                        .await
                        .context("failed to add company")?,
                ),
            };
        }

        if !storage
            .app_repository
            .seen_app(*fingerprint)
            .await
            .context("failed to check whether app was seen")?
        {
            storage
                .app_repository
                .add_app(*fingerprint, details, company_id)
                .await
                .context("failed to register app")?;
            info!("Registered new app: {}", details.display_name());
        }
    }

    let image_path = match screenshot_orchestrator.process_event(&event) {
        Ok(path) => path.map(|path| path.to_string_lossy().into_owned()),
        Err(e) => {
            warn!("Screenshot skipped: {e:#}");
            None
        }
    };

    let app_id = if let Some(fp) = event.data.fingerprint() {
        storage.app_repository.get_app_id(fp).await.ok()
    } else {
        None
    };

    let event_id = storage
        .event_repository
        .add_event(event.as_ref(), app_id, &image_path)
        .await
        .context("failed to persist event")?;

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
