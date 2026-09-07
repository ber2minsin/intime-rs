use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use intime_ai::{
    embedding::EmbeddingServer,
    models::{EmbeddingRequest, EmbeddingResponse, EmbeddingTask, EmbeddingType},
};
use intime_core::{
    features::FeatureFlags,
    models::{AppDetails, Event, EventData, EventMetadata, VersionInfo},
    time::Timestamp,
};
use intime_daemon::{
    CategoryRulesCache, ScreenshotOrchestrator, SessionTracker, handle_incoming_event,
    start_embedding_worker,
};
use intime_platform::{CapturedImage, error::PlatformError, traits::ScreenshotSource};
use intime_storage::testing::TestDatabase;
use tokio::sync::mpsc;

struct FakeCapture {
    captures: Arc<Mutex<Vec<u64>>>,
    fail_handles: Arc<Mutex<Vec<u64>>>,
}

impl ScreenshotSource for FakeCapture {
    fn capture(&mut self, handle: u64) -> Result<CapturedImage, PlatformError> {
        if self.fail_handles.lock().unwrap().contains(&handle) {
            return Err(PlatformError::InvalidWindow);
        }
        self.captures.lock().unwrap().push(handle);
        Ok(CapturedImage {
            width: 2,
            height: 2,
            pixels: vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
        })
    }
}

struct FakeEmbedding {
    calls: Arc<Mutex<Vec<EmbeddingRequest>>>,
    fail_next: Arc<Mutex<bool>>,
}

#[async_trait]
impl EmbeddingServer for FakeEmbedding {
    async fn make_request(
        &mut self,
        req: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, anyhow::Error> {
        if *self.fail_next.lock().unwrap() {
            *self.fail_next.lock().unwrap() = false;
            anyhow::bail!("simulated embedding failure");
        }
        self.calls.lock().unwrap().push(match &req {
            EmbeddingRequest::Text { text } => EmbeddingRequest::Text { text: text.clone() },
            EmbeddingRequest::Image { image_path } => EmbeddingRequest::Image {
                image_path: image_path.clone(),
            },
        });
        Ok(EmbeddingResponse {
            embedding_type: EmbeddingType::Image,
            backend: "fake".into(),
            embedding: (0..512).map(|i| i as f32 * 0.01).collect(),
        })
    }
}

fn app_details() -> AppDetails {
    AppDetails {
        title: "Compose".into(),
        file_path: "/usr/bin/mail".into(),
        aumid: None,
        company_name: Some("MailCorp".into()),
        product_name: Some("Mail".into()),
        version_info: Some(VersionInfo {
            file_version: Some("2.0.0".into()),
            ..Default::default()
        }),
        signature_info: None,
    }
}

struct PipelineHarness {
    db: TestDatabase,
    captures: Arc<Mutex<Vec<u64>>>,
    fail_handles: Arc<Mutex<Vec<u64>>>,
    embed_calls: Arc<Mutex<Vec<EmbeddingRequest>>>,
    fail_embed: Arc<Mutex<bool>>,
    _tmp: tempfile::TempDir,
    old_cwd: PathBuf,
    orchestrator: ScreenshotOrchestrator,
    embed_tx: mpsc::Sender<EmbeddingTask>,
    embed_rx: mpsc::Receiver<EmbeddingTask>,
    flags: FeatureFlags,
    sessions: SessionTracker,
    rules: CategoryRulesCache,
}

impl PipelineHarness {
    async fn new(interval: Duration) -> Self {
        let db = TestDatabase::new().await.expect("db");
        let captures = Arc::new(Mutex::new(Vec::new()));
        let fail_handles = Arc::new(Mutex::new(Vec::new()));
        let embed_calls = Arc::new(Mutex::new(Vec::new()));
        let fail_embed = Arc::new(Mutex::new(false));
        let tmp = tempfile::tempdir().expect("tmpdir");
        let old_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();

        let orchestrator = ScreenshotOrchestrator::with_minimum_interval(
            Box::new(FakeCapture {
                captures: captures.clone(),
                fail_handles: fail_handles.clone(),
            }),
            interval,
        );
        let (embed_tx, embed_rx) = mpsc::channel::<EmbeddingTask>(16);
        let flags = FeatureFlags::default();
        let sessions = SessionTracker::new(flags.clone());
        let rules = CategoryRulesCache::load(&db.storage)
            .await
            .expect("load category rules");
        Self {
            db,
            captures,
            fail_handles,
            embed_calls,
            fail_embed,
            _tmp: tmp,
            old_cwd,
            orchestrator,
            embed_tx,
            embed_rx,
            flags,
            sessions,
            rules,
        }
    }

    async fn handle(&mut self, event: Event) {
        handle_incoming_event(
            Arc::new(event),
            &self.db.storage,
            &mut self.orchestrator,
            self.embed_tx.clone(),
            &self.flags,
            &mut self.sessions,
            &self.rules,
        )
        .await
        .expect("handle event");
    }

    async fn drain_embeddings(&mut self) {
        let storage = self.db.storage.clone();
        let mut fake = FakeEmbedding {
            calls: self.embed_calls.clone(),
            fail_next: self.fail_embed.clone(),
        };
        let (tx, mut rx) = mpsc::channel(8);
        while let Ok(task) = self.embed_rx.try_recv() {
            tx.send(task).await.unwrap();
        }
        drop(tx);
        start_embedding_worker(&mut rx, &mut fake, &storage)
            .await
            .unwrap();
    }
}

impl Drop for PipelineHarness {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.old_cwd);
    }
}

#[tokio::test]
async fn end_to_end_app_seen_focus_screenshot_and_embedding() {
    // Non-zero interval: duplicate same-page focus must not re-capture.
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    let details = app_details();
    let fp = details.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 99,
        },
        metadata: EventMetadata {
            window_title: Some("Compose".into()),
            ..Default::default()
        },
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 99,
        },
        metadata: EventMetadata {
            window_title: Some("Compose".into()),
            focused_element: Some("To".into()),
            ..Default::default()
        },
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 99,
        },
        metadata: EventMetadata {
            window_title: Some("Compose".into()),
            ..Default::default()
        },
    })
    .await;

    let events = h.db.storage.event_repository.list_events(20).await.unwrap();
    assert!(events.iter().any(|e| e.event_type == "app_seen"));
    assert_eq!(
        events
            .iter()
            .filter(|e| e.event_type == "window_focus")
            .count(),
        2
    );
    assert_eq!(h.captures.lock().unwrap().as_slice(), &[99]);

    h.drain_embeddings().await;
    assert_eq!(h.embed_calls.lock().unwrap().len(), 1);
    assert!(h.db.storage.app_repository.seen_app(fp).await.unwrap());
    let company = h
        .db
        .storage
        .app_repository
        .get_app_company(h.db.storage.app_repository.get_app_id(fp).await.unwrap())
        .await
        .unwrap();
    assert_eq!(company.company_name, "MailCorp");
}

#[tokio::test]
async fn idle_events_are_persisted_without_screenshots_or_embeddings() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::IdleStart,
        metadata: Default::default(),
    })
    .await;

    assert!(h.captures.lock().unwrap().is_empty());
    assert!(h.embed_rx.try_recv().is_err());
    let events = h.db.storage.event_repository.list_events(5).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "idle_start");
    assert!(events[0].screenshot_path.is_none());
}

#[tokio::test]
async fn title_change_after_interval_triggers_second_screenshot() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    let details = app_details();
    let fp = details.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 11,
        },
        metadata: Default::default(),
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 11,
        },
        metadata: EventMetadata {
            window_title: Some("Inbox".into()),
            ..Default::default()
        },
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::TitleChange {
            fingerprint: fp,
            new_title: "Writing to Alice".into(),
            window_handle: 11,
        },
        metadata: EventMetadata {
            window_title: Some("Writing to Alice".into()),
            ..Default::default()
        },
    })
    .await;

    // AppSeen (no title) + Focus (Inbox) + TitleChange all capture when interval is 0.
    assert_eq!(h.captures.lock().unwrap().as_slice(), &[11, 11, 11]);
    h.drain_embeddings().await;
    assert_eq!(h.embed_calls.lock().unwrap().len(), 3);

    let events = h.db.storage.event_repository.list_events(10).await.unwrap();
    let title = events
        .iter()
        .find(|e| e.event_type == "title_change")
        .expect("title change row");
    assert!(title.screenshot_path.is_some());
    let payload: Event = serde_json::from_str(title.payload.as_ref().unwrap()).unwrap();
    assert_eq!(payload.metadata.window_title.as_deref(), Some("Writing to Alice"));
}

#[tokio::test]
async fn new_window_handle_captures_immediately() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    let details = app_details();
    let fp = details.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 1,
        },
        metadata: Default::default(),
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 1,
        },
        metadata: EventMetadata {
            window_title: Some("A".into()),
            ..Default::default()
        },
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 2,
        },
        metadata: EventMetadata {
            window_title: Some("B".into()),
            ..Default::default()
        },
    })
    .await;

    assert_eq!(h.captures.lock().unwrap().as_slice(), &[1, 2]);
}

#[tokio::test]
async fn screenshot_failure_still_persists_event() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    let details = app_details();
    let fp = details.fingerprint();
    h.fail_handles.lock().unwrap().push(77);

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 77,
        },
        metadata: Default::default(),
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 77,
        },
        metadata: EventMetadata {
            window_title: Some("Broken".into()),
            ..Default::default()
        },
    })
    .await;

    assert!(h.captures.lock().unwrap().is_empty());
    assert!(h.embed_rx.try_recv().is_err());
    let events = h.db.storage.event_repository.list_events(10).await.unwrap();
    assert!(events.iter().any(|e| e.event_type == "window_focus"));
    assert!(
        events
            .iter()
            .filter(|e| e.event_type == "window_focus")
            .all(|e| e.screenshot_path.is_none())
    );
}

#[tokio::test]
async fn embedding_worker_continues_after_transient_failure() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    let details = app_details();
    let fp = details.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 5,
        },
        metadata: Default::default(),
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 5,
        },
        metadata: EventMetadata {
            window_title: Some("One".into()),
            ..Default::default()
        },
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::TitleChange {
            fingerprint: fp,
            new_title: "Two".into(),
            window_handle: 5,
        },
        metadata: EventMetadata {
            window_title: Some("Two".into()),
            ..Default::default()
        },
    })
    .await;

    // Three screenshot events queue three embedding tasks; first fails, others succeed.
    assert_eq!(h.captures.lock().unwrap().len(), 3);
    *h.fail_embed.lock().unwrap() = true;
    h.drain_embeddings().await;
    assert_eq!(h.embed_calls.lock().unwrap().len(), 2);

    let embedding_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM embedding")
            .fetch_one(&h.db.pool)
            .await
            .unwrap();
    assert_eq!(embedding_count, 2);
}

#[tokio::test]
async fn text_changed_and_background_events_persist_without_capture() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    let details = app_details();
    let fp = details.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 3,
        },
        metadata: Default::default(),
    })
    .await;

    // AppSeen has a window handle so it may capture once; events without handles must not.
    assert_eq!(h.captures.lock().unwrap().as_slice(), &[3]);

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::Background { fingerprint: fp },
        metadata: Default::default(),
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::IdleEnd,
        metadata: Default::default(),
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::Gap,
        metadata: Default::default(),
    })
    .await;

    assert_eq!(h.captures.lock().unwrap().as_slice(), &[3]);
    let types: Vec<_> = h
        .db
        .storage
        .event_repository
        .list_events(20)
        .await
        .unwrap()
        .into_iter()
        .map(|e| e.event_type)
        .collect();
    assert!(types.contains(&"background".to_string()));
    assert!(types.contains(&"idle_end".to_string()));
    assert!(types.contains(&"gap".to_string()));
}

#[tokio::test]
async fn ui_action_form_submit_persists_as_raw_event() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;
    let details = app_details();
    let fp = details.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 11,
        },
        metadata: EventMetadata {
            window_title: Some("Login".into()),
            ..Default::default()
        },
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 11,
        },
        metadata: EventMetadata {
            window_title: Some("Login".into()),
            ..Default::default()
        },
    })
    .await;
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::FormSubmit,
            fingerprint: fp,
            window_handle: 11,
            label: Some("Sign in".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Login".into()),
            focused_element: Some("Sign in".into()),
            focused_control_type: Some("PushButton".into()),
            ..Default::default()
        },
    })
    .await;

    let events = h.db.storage.event_repository.list_events(20).await.unwrap();
    let submit = events
        .iter()
        .find(|e| e.event_type == "ui_action")
        .expect("ui_action event");
    assert_eq!(submit.focused_element.as_deref(), Some("Sign in"));
    let payload = submit.payload.as_deref().unwrap_or("");
    assert!(payload.contains("form_submit") || payload.contains("FormSubmit"));

    let action_table: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='action'",
    )
    .fetch_one(&h.db.pool)
    .await
    .unwrap();
    assert_eq!(action_table, 0, "action table should be dropped");
}

#[tokio::test]
async fn repeated_app_seen_does_not_duplicate_app_row() {
    let mut h = PipelineHarness::new(Duration::from_millis(0)).await;
    let details = app_details();
    let fp = details.fingerprint();

    for _ in 0..3 {
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::AppSeen {
                fingerprint: fp,
                details: details.clone(),
                window_handle: 9,
            },
            metadata: Default::default(),
        })
        .await;
    }

    let app_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM app")
        .fetch_one(&h.db.pool)
        .await
        .unwrap();
    assert_eq!(app_count, 1);
    let company_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM company")
        .fetch_one(&h.db.pool)
        .await
        .unwrap();
    assert_eq!(company_count, 1);
}
