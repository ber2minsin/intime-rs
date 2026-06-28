use std::sync::Arc;

use anyhow::Error;
use intime_ai::{
    embedding::EmbeddingServer,
    models::{EmbeddingRequest, EmbeddingTask},
};
use intime_core::models::{Event, EventData};
use intime_storage::storage::Storage;
use tokio::sync::mpsc;
use tracing::info;

use crate::orchestrator::ScreenshotOrchestrator;

/// Core logic for handling an event without crashing the thread
pub async fn handle_incoming_event(
    event: Arc<Event>,
    storage: &Storage,
    screenshot_orchestrator: &mut ScreenshotOrchestrator,
    embedding_queue: mpsc::Sender<EmbeddingTask>,
) -> Result<(), Error> {
    // handle registration/metadata if AppSeen event
    if let EventData::AppSeen {
        fingerprint,
        details,
        ..
    } = &event.data
    {
        // get company
        let mut company_id = None;
        if let Some(name) = details.company() {
            company_id = match storage.app_repository.seen_company(&name).await {
                Ok(id) => Some(id),
                Err(_) => Some(storage.app_repository.add_company(&name).await?),
            };
        }

        // get app
        if !storage.app_repository.seen_app(*fingerprint).await? {
            storage
                .app_repository
                .add_app(*fingerprint, details, company_id)
                .await?;
            info!("Registered new app: {}", details.display_name());
        }
    }
    // 2. UNIFIED SCREENSHOT TRIGGER
    // Executes on ANY event that yields both a fingerprint and an active window handle
    // --- Screenshot + embedding pipeline ---
    let mut image_path: Option<String> = None;
    match &event.as_ref().data {
        EventData::WindowFocus { window_handle, .. }
        | EventData::TitleChange { window_handle, .. } => {
            image_path = match screenshot_orchestrator.process(*window_handle) {
                Ok(p) => Some(p.to_string_lossy().into_owned()),
                Err(_) => return Ok(()),
            };
        }
        _ => {}
    };

    // Now, extract the ID and add the event to the timeline
    let app_id = if let Some(fp) = event.data.fingerprint() {
        storage.app_repository.get_app_id(fp).await.ok()
    } else {
        None
    };

    let event_id = storage
        .event_repository
        .add_event(event.as_ref(), app_id, &image_path)
        .await?;

    let req = EmbeddingRequest::Image {
        image_path: image_path.as_ref().unwrap().to_string(),
    };

    let task = EmbeddingTask { event_id, req };
    embedding_queue.send(task).await?;

    Ok(())
}

pub async fn start_embedding_worker(
    rx: &mut mpsc::Receiver<EmbeddingTask>,
    service: &mut (dyn EmbeddingServer + Send),
    storage: &Storage,
) -> Result<(), Error> {
    while let Some(task) = rx.recv().await {
        match service.make_request(task.req).await {
            Ok(resp) => {
                storage
                    .embedding_repository
                    .add_embedding(task.event_id, resp)
                    .await?;
            }
            Err(e) => return Err(e),
        }
    }

    Ok(())
}
