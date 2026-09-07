use anyhow::{Context, Result};
use intime_ai::{embedding::EmbeddingService, models::EmbeddingTask};
use intime_core::{
    features::FeatureFlags,
    models::{Event, EventData, EventMetadata},
    retention::ScreenshotRetentionPolicy,
    time::Timestamp,
};
use intime_daemon::{
    CategoryRulesCache, HeartbeatOutcome, IdleChecker, IdleTransition, ScreenshotOrchestrator,
    SessionTracker, handle_heartbeat_capture, handle_incoming_event, run_screenshot_retention_loop,
    start_embedding_worker,
};
use intime_platform::{create_capture_engine, create_event_source};
use intime_storage::{config::StorageConfig, storage::Storage};
use std::{env, sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, level_filters::LevelFilter, warn};
use tracing_subscriber::{
    Layer, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt as _,
};

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

fn idle_marker(data: EventData) -> Arc<Event> {
    Arc::new(Event {
        timestamp: Timestamp::now(),
        data,
        metadata: EventMetadata::default(),
    })
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

    let flags = FeatureFlags::from_env();
    info!(?flags, "Loaded feature flags");

    let retention_policy = ScreenshotRetentionPolicy::from_env();
    info!(?retention_policy, "Screenshot retention policy");

    let idle_checker = IdleChecker::from_env();
    info!(
        idle_after_secs = idle_checker.timeout().as_secs(),
        "Idle checker ready"
    );

    let rules = CategoryRulesCache::load(&storage)
        .await
        .context("failed to load category activity rules")?;
    info!("Loaded {} activity rules", rules.len());

    let embedding_server_url =
        env::var("EMBEDDING_SERVER_URL").unwrap_or_else(|_| "http://localhost:8000".to_string());

    let local_set = tokio::task::LocalSet::new();

    let (evttx, _) = broadcast::channel::<Arc<Event>>(1024);
    let (embedtx, mut embedrx) = mpsc::channel::<EmbeddingTask>(64);

    let tracker_storage = storage.clone();
    let tracker_flags = flags.clone();
    let mut tracker_rx = evttx.subscribe();
    let tracker_handle = local_set.spawn_local(async move {
        info!("Tracker task started");
        let mut sessions = SessionTracker::new(tracker_flags.clone());
        let mut idle = idle_checker;
        // Re-capture the last focused page while it stays open (YouTube watch, etc.).
        let mut last_focus: Option<Arc<Event>> = None;
        let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Skip the immediate first tick so we don't double-fire on startup.
        heartbeat.tick().await;

        loop {
            tokio::select! {
                result = tracker_rx.recv() => {
                    let Ok(event) = result else { break };

                    if let Some(IdleTransition::BecameActive) = idle.observe_event(&event) {
                        info!("User activity resumed; ending idle");
                        if let Err(e) = handle_incoming_event(
                            idle_marker(EventData::IdleEnd),
                            &tracker_storage,
                            &mut screenshot_orchestrator,
                            embedtx.clone(),
                            &tracker_flags,
                            &mut sessions,
                            &rules,
                        )
                        .await
                        {
                            error!("Failed to process idle_end: {e:#}");
                        }
                    }

                    if is_heartbeat_source(&event) {
                        last_focus = Some(event.clone());
                    }
                    if let Err(e) = handle_incoming_event(
                        event,
                        &tracker_storage,
                        &mut screenshot_orchestrator,
                        embedtx.clone(),
                        &tracker_flags,
                        &mut sessions,
                        &rules,
                    )
                    .await
                    {
                        error!("Failed to process event: {e:#}");
                    }
                }
                _ = heartbeat.tick() => {
                    if let Some(IdleTransition::BecameIdle) = idle.poll() {
                        info!("Idle timeout reached; stopping screenshots");
                        last_focus = None;
                        if let Err(e) = handle_incoming_event(
                            idle_marker(EventData::IdleStart),
                            &tracker_storage,
                            &mut screenshot_orchestrator,
                            embedtx.clone(),
                            &tracker_flags,
                            &mut sessions,
                            &rules,
                        )
                        .await
                        {
                            error!("Failed to process idle_start: {e:#}");
                        }
                        continue;
                    }

                    if idle.is_idle() {
                        continue;
                    }

                    let Some(event) = last_focus.clone() else { continue };
                    match handle_heartbeat_capture(
                        event,
                        &tracker_storage,
                        &mut screenshot_orchestrator,
                        embedtx.clone(),
                        &tracker_flags,
                        &sessions,
                    )
                    .await
                    {
                        Ok(HeartbeatOutcome::Captured) | Ok(HeartbeatOutcome::Skipped) => {}
                        Ok(HeartbeatOutcome::StaleFocus) => {
                            last_focus = None;
                        }
                        Err(e) => {
                            error!("Failed to process heartbeat capture: {e:#}");
                        }
                    }
                }
            }
        }
    });

    let embedding_storage = storage.clone();
    let embeddings_enabled = flags.embeddings_enabled;
    let embedding_handle = tokio::spawn(async move {
        if !embeddings_enabled {
            info!("Embeddings disabled; worker idle");
            while embedrx.recv().await.is_some() {}
            return;
        }
        let mut embedding_service = match EmbeddingService::new(&embedding_server_url) {
            Ok(s) => s,
            Err(e) => {
                error!("invalid EMBEDDING_SERVER_URL: {e:#}");
                return;
            }
        };
        if let Err(e) =
            start_embedding_worker(&mut embedrx, &mut embedding_service, &embedding_storage).await
        {
            error!("Embedding worker exited: {e:#}");
        }
    });

    let retention_storage = storage.clone();
    let retention_handle = tokio::spawn(async move {
        run_screenshot_retention_loop(retention_storage, retention_policy).await;
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

    let (platform_result, _, _, _, _) = local_set
        .run_until(async move {
            tokio::try_join!(
                platform_handle,
                tracker_handle,
                display_handle,
                embedding_handle,
                retention_handle
            )
        })
        .await?;

    platform_result.context("platform event loop failed")?;
    Ok(())
}

fn is_heartbeat_source(event: &Event) -> bool {
    matches!(
        event.data,
        EventData::WindowFocus { .. }
            | EventData::TitleChange { .. }
            | EventData::TextChanged { .. }
            | EventData::AppSeen { .. }
    ) && event.data.window_handle().is_some()
}
