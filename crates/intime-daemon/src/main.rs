use anyhow::{Context, Result};
use intime_ai::{embedding::EmbeddingService, models::EmbeddingTask};
use intime_core::models::Event;
use intime_platform::{create_capture_engine, create_event_source};
use intime_storage::{config::StorageConfig, storage::Storage};
use std::{env, sync::Arc};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::{
    Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt as _,
};

use crate::{orchestrator::ScreenshotOrchestrator, pipeline::{handle_incoming_event, start_embedding_worker}};
mod orchestrator;
mod pipeline;

fn setup_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = tracing_appender::rolling::never(".", "intime-daemon.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_writer(file_writer)
        .with_filter(LevelFilter::INFO);

    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(LevelFilter::INFO);

    Registry::default()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    guard // Return it here
}

#[tokio::main]
async fn main() -> Result<()> {
    // Setup Environment and Logging
    dotenvy::dotenv().ok();

    let _log_guard = setup_tracing();
    // Initialize ScreenshotOrchestrator
    let capture_engine = create_capture_engine().expect("Capture engine could not be created");
    let mut screenshot_orchestrator = ScreenshotOrchestrator::new(capture_engine);

    // Initialize Storage
    let storage_config = StorageConfig {
        database_file: env::var("DATABASE_FILE").context("DATABASE_FILE must be set")?,
    };
    let storage = Storage::connect(&storage_config)
        .await
        .context("Failed to connect to storage")?;

    let embedding_server_url = "http://localhost:8000";

    let local_set = tokio::task::LocalSet::new();

    // Setup Communication Channels
    let (evttx, _) = broadcast::channel::<Arc<Event>>(1024);

    let (embedtx, mut embedrx) = mpsc::channel::<EmbeddingTask>(64);

    // Spawn Tracker Task (Data Persistence)
    // 2. Spawn Tracker Task using local_set.spawn_local instead of tokio::spawn
    let tracker_storage = storage.clone();
    let mut tracker_rx = evttx.subscribe();
    let tracker_handle = local_set.spawn_local(async move {
        info!("Tracker task started");
        while let Ok(event) = tracker_rx.recv().await {
            if let Err(e) = handle_incoming_event(
                event,
                &tracker_storage,
                &mut screenshot_orchestrator,
                embedtx.clone(),
            )
            .await
            {
                error!("Failed to process event: {e:?}");
            }
        }
    });

    let embedding_storage = storage.clone();
    let mut embedding_service = EmbeddingService::new(embedding_server_url.to_string());
    let embedding_handle = tokio::spawn(async move {
       start_embedding_worker(&mut embedrx, &mut embedding_service, &embedding_storage).await.unwrap();
    });
    // Spawn Display Task (This one can stay standard)
    let mut display_rx = evttx.subscribe();
    let display_handle = tokio::spawn(async move {
        while let Ok(event) = display_rx.recv().await {
            info!(target: "activity", "{:?}", event);
        }
    });

    // Spawn Platform Event Source
    let producer_tx = evttx.clone();
    let platform_handle = tokio::task::spawn_blocking(move || {
        let mut event_source = create_event_source().expect("Could not initialize EventSource");
        info!("Platform event polling started (Blocking Thread)");

        while let Ok(event) = event_source.poll() {
            if let Err(e) = producer_tx.send(Arc::new(event)) {
                error!("Broadcast channel full or closed: {e}");
                // Explicitly return an error instead of breaking
                return Err(anyhow::anyhow!("Broadcast channel closed: {}", e));
            }
        }

        // Explicit, clear return for a successful loop exhaustion
        Ok(())
    });

    let (platform_result, _, _, _) = local_set
        .run_until(async move { tokio::try_join!(platform_handle, tracker_handle, display_handle, embedding_handle) })
        .await?; // 1. Clears the JoinError (checks if tasks crashed)

    platform_result.context("Platform event loop encountered a critical error")?;

    Ok(())
}

