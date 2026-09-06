use anyhow::{Context, Result};
use intime_ai::{embedding::EmbeddingService, models::EmbeddingTask};
use intime_core::models::Event;
use intime_platform::{create_capture_engine, create_event_source};
use intime_storage::{config::StorageConfig, storage::Storage};
use std::{env, sync::Arc};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, level_filters::LevelFilter, warn};
use tracing_subscriber::{
    Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt as _,
};

use crate::{
    orchestrator::ScreenshotOrchestrator,
    pipeline::{handle_incoming_event, start_embedding_worker},
};

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

    guard
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let _log_guard = setup_tracing();

    let capture_engine =
        create_capture_engine().context("failed to create screenshot capture engine")?;
    let mut screenshot_orchestrator = ScreenshotOrchestrator::new(capture_engine);

    let storage_config = StorageConfig {
        database_file: env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
    };
    let storage = Storage::connect(&storage_config)
        .await
        .context("failed to connect to storage")?;

    let embedding_server_url =
        env::var("EMBEDDING_SERVER_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

    let local_set = tokio::task::LocalSet::new();

    let (evttx, _) = broadcast::channel::<Arc<Event>>(1024);
    let (embedtx, mut embedrx) = mpsc::channel::<EmbeddingTask>(64);

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
                error!("Failed to process event: {e:#}");
            }
        }
    });

    let embedding_storage = storage.clone();
    let mut embedding_service =
        EmbeddingService::new(&embedding_server_url).context("invalid EMBEDDING_SERVER_URL")?;
    let embedding_handle = tokio::spawn(async move {
        if let Err(e) =
            start_embedding_worker(&mut embedrx, &mut embedding_service, &embedding_storage).await
        {
            error!("Embedding worker exited: {e:#}");
        }
    });

    let mut display_rx = evttx.subscribe();
    let display_handle = tokio::spawn(async move {
        while let Ok(event) = display_rx.recv().await {
            info!(target: "activity", "{:?}", event);
        }
    });

    let producer_tx = evttx.clone();
    let platform_handle = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut event_source =
            create_event_source().context("failed to initialize platform event source")?;
        info!("Platform event polling started");

        loop {
            match event_source.poll() {
                Ok(event) => {
                    if let Err(e) = producer_tx.send(Arc::new(event)) {
                        return Err(anyhow::anyhow!("broadcast channel closed: {e}"));
                    }
                }
                Err(e) => {
                    warn!("Event source poll error: {e}");
                    return Err(e.into());
                }
            }
        }
    });

    let (platform_result, _, _, _) = local_set
        .run_until(async move {
            tokio::try_join!(
                platform_handle,
                tracker_handle,
                display_handle,
                embedding_handle
            )
        })
        .await?;

    platform_result.context("platform event loop failed")?;
    Ok(())
}
