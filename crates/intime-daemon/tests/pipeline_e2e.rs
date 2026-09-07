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
    assert_eq!(submit.event_type, "ui_action");
    let decoded: Event = serde_json::from_str(submit.payload.as_deref().unwrap()).unwrap();
    assert_eq!(decoded.metadata.focused_element.as_deref(), Some("Sign in"));
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

#[tokio::test]
async fn mpris_play_attaches_browser_app_and_merges_youtube_session() {
    // Regression: MPRIS fingerprints are synthetic, so play_media used to open a
    // media session with app_id=NULL and a divergent context key ("… - brave"),
    // then the Brave tab focus created a second session for the same video.
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let brave = AppDetails {
        title: "(1) Stop Playing Kayle Reroll, Play This Instead - YouTube - Brave".into(),
        file_path: "/opt/brave.com/brave/brave".into(),
        aumid: Some("brave-browser".into()),
        company_name: Some("Brave Software".into()),
        product_name: Some("brave-browser".into()),
        version_info: None,
        signature_info: None,
    };
    let brave_fp = brave.fingerprint();
    let mpris_fp = blake3::hash(b"mpris\x1fbrave");

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: brave_fp,
            details: brave.clone(),
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some(brave.title.clone()),
            executable_path: Some(brave.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    let brave_app_id = h
        .db
        .storage
        .app_repository
        .get_app_id(brave_fp)
        .await
        .unwrap();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::PlayMedia,
            fingerprint: mpris_fp,
            window_handle: 999,
            label: Some("Stop Playing Kayle Reroll, Play This Instead".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Stop Playing Kayle Reroll, Play This Instead - YouTube".into()),
            url: Some("https://www.youtube.com/watch?v=abc123".into()),
            focused_element: Some("Playing".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        },
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: brave_fp,
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some(
                "(1) Stop Playing Kayle Reroll, Play This Instead - YouTube - Brave".into(),
            ),
            executable_path: Some(brave.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    let sessions: Vec<(i64, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, app_id, intent, context_key FROM session ORDER BY id",
    )
    .fetch_all(&h.db.pool)
    .await
    .unwrap();

    assert_eq!(
        sessions.len(),
        1,
        "expected one merged YouTube session, got {sessions:?}"
    );
    let (_id, app_id, intent, context_key) = &sessions[0];
    assert_eq!(app_id.as_ref(), Some(&brave_app_id), "MPRIS play must resolve Brave app_id");
    assert_eq!(intent.as_deref(), Some("media_streaming_official"));
    assert_eq!(
        context_key.as_deref(),
        Some("media:yt:abc123"),
        "watch URL id must define the session key, got {context_key:?}"
    );

    let play_event = h
        .db
        .storage
        .event_repository
        .list_events(20)
        .await
        .unwrap()
        .into_iter()
        .find(|e| e.event_type == "ui_action")
        .expect("play_media event");
    assert_eq!(
        play_event.app_id,
        Some(brave_app_id),
        "persisted MPRIS event must carry Brave app_id"
    );
}

#[tokio::test]
async fn idle_start_closes_session_with_ended_reason() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let brave = AppDetails {
        title: "Cool Video - YouTube - Brave".into(),
        file_path: "/opt/brave.com/brave/brave".into(),
        aumid: Some("brave-browser".into()),
        company_name: Some("Brave Software".into()),
        product_name: Some("brave-browser".into()),
        version_info: None,
        signature_info: None,
    };
    let brave_fp = brave.fingerprint();
    let mpris_fp = blake3::hash(b"mpris\x1fbrave");

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: brave_fp,
            details: brave.clone(),
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some(brave.title.clone()),
            executable_path: Some(brave.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::PlayMedia,
            fingerprint: mpris_fp,
            window_handle: 999,
            label: Some("Cool Video".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Cool Video - YouTube".into()),
            url: Some("https://www.youtube.com/watch?v=idle1".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        },
    })
    .await;

    let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM session WHERE ended_at IS NULL")
        .fetch_one(&h.db.pool)
        .await
        .unwrap();
    assert_eq!(open, 1, "expected an open session before idle");

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::IdleStart,
        metadata: Default::default(),
    })
    .await;

    let rows: Vec<(Option<String>,)> =
        sqlx::query_as("SELECT ended_reason FROM session WHERE ended_at IS NOT NULL")
            .fetch_all(&h.db.pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0.as_deref(), Some("idle"));

    let idle_events = h
        .db
        .storage
        .event_repository
        .list_events(50)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.event_type == "idle_start")
        .count();
    assert_eq!(idle_events, 1);
}

#[tokio::test]
async fn distinct_videos_netflix_and_search_open_separate_sessions() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let brave = AppDetails {
        title: "Brave".into(),
        file_path: "/opt/brave.com/brave/brave".into(),
        aumid: Some("brave-browser".into()),
        company_name: Some("Brave Software".into()),
        product_name: Some("brave-browser".into()),
        version_info: None,
        signature_info: None,
    };
    let fp = brave.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: brave.clone(),
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some(brave.title.clone()),
            executable_path: Some(brave.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    // Video A
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::PlayMedia,
            fingerprint: blake3::hash(b"mpris\x1fbrave"),
            window_handle: 999,
            label: Some("Video Alpha".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Video Alpha - YouTube - Brave".into()),
            url: Some("https://www.youtube.com/watch?v=videoAAAA".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        },
    })
    .await;

    // Video B (different id → new session)
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::PlayMedia,
            fingerprint: blake3::hash(b"mpris\x1fbrave"),
            window_handle: 999,
            label: Some("Video Beta".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Video Beta - YouTube - Brave".into()),
            url: Some("https://www.youtube.com/watch?v=videoBBBB".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        },
    })
    .await;

    // Netflix show
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::PlayMedia,
            fingerprint: blake3::hash(b"mpris\x1fbrave"),
            window_handle: 999,
            label: Some("Community".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Community - Netflix - Brave".into()),
            url: Some("https://www.netflix.com/title/70136141".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        },
    })
    .await;

    // Google search — need a few meaningful events to promote
    for i in 0..3 {
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: fp,
                window_handle: 80 + i,
            },
            metadata: EventMetadata {
                window_title: Some("good ai UI designers - Google Search - Brave".into()),
                url: Some("https://www.google.com/search?q=good+ai+UI+designers".into()),
                executable_path: Some(brave.file_path.clone()),
                ..Default::default()
            },
        })
        .await;
    }

    let sessions: Vec<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT intent, context_key FROM session ORDER BY id",
    )
    .fetch_all(&h.db.pool)
    .await
    .unwrap();

    assert!(
        sessions.len() >= 4,
        "expected ≥4 sessions (2 yt + netflix + search), got {sessions:?}"
    );
    let intents: Vec<&str> = sessions
        .iter()
        .filter_map(|(i, _)| i.as_deref())
        .collect();
    assert!(
        intents.iter().any(|i| *i == "media_streaming_official"),
        "missing streaming session: {sessions:?}"
    );
    assert!(
        intents.iter().any(|i| *i == "web_search"),
        "missing web_search session: {sessions:?}"
    );
    let keys: Vec<&str> = sessions
        .iter()
        .filter_map(|(_, k)| k.as_deref())
        .collect();
    assert!(
        keys.iter().any(|k| k.contains("videoAAAA") || *k == "media:yt:videoAAAA"),
        "missing video A key: {sessions:?}"
    );
    assert!(
        keys.iter().any(|k| k.contains("videoBBBB") || *k == "media:yt:videoBBBB"),
        "missing video B key: {sessions:?}"
    );
    assert!(
        keys.iter().any(|k| *k == "media:nf:70136141"),
        "missing netflix key: {sessions:?}"
    );
}

#[tokio::test]
async fn finish_from_other_app_does_not_clear_media_session() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let brave = AppDetails {
        title: "Show - Netflix - Brave".into(),
        file_path: "/opt/brave.com/brave/brave".into(),
        aumid: Some("brave-browser".into()),
        company_name: Some("Brave Software".into()),
        product_name: Some("brave-browser".into()),
        version_info: None,
        signature_info: None,
    };
    let brave_fp = brave.fingerprint();
    let bt_fp = blake3::hash(b"bluetooth-devices");

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: brave_fp,
            details: brave.clone(),
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some(brave.title.clone()),
            executable_path: Some(brave.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::PlayMedia,
            fingerprint: blake3::hash(b"mpris\x1fbrave"),
            window_handle: 999,
            label: Some("Community".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Community - Netflix - Brave".into()),
            url: Some("https://www.netflix.com/title/70136141".into()),
            focused_control_type: Some("mpris".into()),
            automation_id: Some("brave".into()),
            ..Default::default()
        },
    })
    .await;

    let open_before: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session WHERE ended_at IS NULL")
            .fetch_one(&h.db.pool)
            .await
            .unwrap();
    assert_eq!(open_before, 1);

    // Bluetooth dialog closes — must not end the Netflix session.
    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::UiAction {
            kind: intime_core::models::UiActionKind::Finish,
            fingerprint: bt_fp,
            window_handle: 12,
            label: Some("Bluetooth Devices".into()),
        },
        metadata: EventMetadata {
            window_title: Some("Bluetooth Devices".into()),
            ..Default::default()
        },
    })
    .await;

    let open_after: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session WHERE ended_at IS NULL")
            .fetch_one(&h.db.pool)
            .await
            .unwrap();
    assert_eq!(
        open_after, 1,
        "Finish from another app must not close the media session"
    );
}

#[tokio::test]
async fn coding_workspace_merges_files_into_one_session() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let cursor = AppDetails {
        title: "pipeline.rs — intime-rs — Cursor".into(),
        file_path: "/opt/Cursor/cursor".into(),
        aumid: Some("cursor".into()),
        company_name: Some("Anysphere".into()),
        product_name: Some("Cursor".into()),
        version_info: None,
        signature_info: None,
    };
    let fp = cursor.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: cursor.clone(),
            window_handle: 10,
        },
        metadata: EventMetadata {
            window_title: Some(cursor.title.clone()),
            executable_path: Some(cursor.file_path.clone()),
            document_name: Some("pipeline.rs".into()),
            workspace_path: Some("intime-rs".into()),
            ..Default::default()
        },
    })
    .await;

    for (i, file) in ["pipeline.rs", "session_tracker.rs", "category.rs"]
        .iter()
        .enumerate()
    {
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: fp,
                window_handle: 10 + i as u64,
            },
            metadata: EventMetadata {
                window_title: Some(format!("{file} — intime-rs — Cursor")),
                executable_path: Some(cursor.file_path.clone()),
                document_name: Some((*file).into()),
                workspace_path: Some("intime-rs".into()),
                ..Default::default()
            },
        })
        .await;
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::TitleChange {
                fingerprint: fp,
                new_title: format!("{file} — intime-rs — Cursor"),
                window_handle: 10,
            },
            metadata: EventMetadata {
                window_title: Some(format!("{file} — intime-rs — Cursor")),
                document_name: Some((*file).into()),
                workspace_path: Some("intime-rs".into()),
                ..Default::default()
            },
        })
        .await;
    }

    let sessions: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT intent, context_key, summary FROM session ORDER BY id",
    )
    .fetch_all(&h.db.pool)
    .await
    .unwrap();
    assert_eq!(sessions.len(), 1, "expected one workspace session: {sessions:?}");
    let (intent, key, _) = &sessions[0];
    assert!(
        intent.as_deref() == Some("ai_coding") || intent.as_deref() == Some("code_editing"),
        "unexpected coding intent: {sessions:?}"
    );
    assert_eq!(key.as_deref(), Some("ws:intime-rs"));
}

#[tokio::test]
async fn github_repo_pages_share_repo_context_and_code_category() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let brave = AppDetails {
        title: "brave".into(),
        file_path: "/opt/brave.com/brave/brave".into(),
        aumid: Some("brave-browser".into()),
        company_name: Some("Brave Software".into()),
        product_name: Some("brave-browser".into()),
        version_info: None,
        signature_info: None,
    };
    let fp = brave.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: brave.clone(),
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some("ber2minsin/intime-rs".into()),
            executable_path: Some(brave.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    for (path, title) in [
        (
            "https://github.com/ber2minsin/intime-rs",
            "ber2minsin/intime-rs · GitHub - Brave",
        ),
        (
            "https://github.com/ber2minsin/intime-rs/pull/3",
            "Fix sessions by ber2minsin · Pull Request #3 · ber2minsin/intime-rs · GitHub - Brave",
        ),
        (
            "https://github.com/ber2minsin/intime-rs/blob/main/README.md",
            "intime-rs/README.md at main · ber2minsin/intime-rs · GitHub - Brave",
        ),
    ] {
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: fp,
                window_handle: 80,
            },
            metadata: EventMetadata {
                window_title: Some(title.into()),
                url: Some(path.into()),
                executable_path: Some(brave.file_path.clone()),
                ..Default::default()
            },
        })
        .await;
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::TitleChange {
                fingerprint: fp,
                new_title: title.into(),
                window_handle: 80,
            },
            metadata: EventMetadata {
                window_title: Some(title.into()),
                url: Some(path.into()),
                executable_path: Some(brave.file_path.clone()),
                ..Default::default()
            },
        })
        .await;
    }

    let sessions: Vec<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT intent, context_key FROM session ORDER BY id")
            .fetch_all(&h.db.pool)
            .await
            .unwrap();
    assert_eq!(sessions.len(), 1, "repo pages should merge: {sessions:?}");
    assert_eq!(
        sessions[0].0.as_deref(),
        Some("code_collaboration"),
        "{sessions:?}"
    );
    assert_eq!(
        sessions[0].1.as_deref(),
        Some("repo:github.com:ber2minsin/intime-rs")
    );
}

#[tokio::test]
async fn steam_game_session_and_list_sessions_query() {
    let mut h = PipelineHarness::new(Duration::from_secs(60)).await;
    h.flags.screenshots_enabled = false;
    h.flags.embeddings_enabled = false;

    let steam = AppDetails {
        title: "Counter-Strike 2".into(),
        file_path: "/home/user/.local/share/Steam/steamapps/common/Counter-Strike Global Offensive/game/bin/linuxsteamrt64/cs2"
            .into(),
        aumid: Some("steam".into()),
        company_name: Some("Valve".into()),
        product_name: Some("steam".into()),
        version_info: None,
        signature_info: None,
    };
    let fp = steam.fingerprint();

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::AppSeen {
            fingerprint: fp,
            details: steam.clone(),
            window_handle: 42,
        },
        metadata: EventMetadata {
            window_title: Some(steam.title.clone()),
            executable_path: Some(steam.file_path.clone()),
            ..Default::default()
        },
    })
    .await;

    for _ in 0..3 {
        h.handle(Event {
            timestamp: Timestamp::now(),
            data: EventData::WindowFocus {
                fingerprint: fp,
                window_handle: 42,
            },
            metadata: EventMetadata {
                window_title: Some("Counter-Strike 2".into()),
                executable_path: Some(steam.file_path.clone()),
                ..Default::default()
            },
        })
        .await;
    }

    h.handle(Event {
        timestamp: Timestamp::now(),
        data: EventData::IdleStart,
        metadata: Default::default(),
    })
    .await;

    let sessions: Vec<(Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT intent, context_key, summary FROM session ORDER BY id",
    )
    .fetch_all(&h.db.pool)
    .await
    .unwrap();
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    assert_eq!(sessions[0].0.as_deref(), Some("gaming"));
    assert_eq!(sessions[0].1.as_deref(), Some("game:counter-strike 2"));
    assert!(
        sessions[0]
            .2
            .as_deref()
            .is_some_and(|s| s.contains("gaming") && s.contains("game:counter-strike 2")),
        "summary missing: {sessions:?}"
    );

    use intime_storage::SessionListFilter;
    let listed = h
        .db
        .storage
        .session_repository
        .list_sessions(SessionListFilter {
            intent: Some("gaming".into()),
            context_key_prefix: Some("game:".into()),
            limit: 10,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
}
