use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result, bail};
use intime_ai::embedding::EmbeddingService;
use intime_core::{
    features::FeatureFlags,
    models::{AppDetails, Event, EventData, EventMetadata},
    time::Timestamp,
};
use intime_daemon::{
    CategoryRulesCache, ScreenshotOrchestrator, SessionTracker, handle_incoming_event,
    start_embedding_worker,
};
use intime_storage::testing::TestDatabase;
use tokio::sync::mpsc;

use crate::{
    capture::FixtureCapture,
    scenario::{Scenario, ScenarioStep},
};

#[derive(Debug)]
pub struct ReplayReport {
    pub event_count: usize,
    pub screenshot_count: usize,
    pub embedding_count: i64,
    pub database_url: String,
    pub work_dir: PathBuf,
}

pub async fn replay_scenario(
    scenario_dir: &Path,
    embedding_server_url: &str,
) -> Result<ReplayReport> {
    let scenario_path = scenario_dir.join("scenario.json");
    let scenario = Scenario::load(&scenario_path)
        .with_context(|| format!("load {}", scenario_path.display()))?;

    let db = TestDatabase::new().await.context("test database")?;
    let work = tempfile::tempdir().context("workdir")?;
    let old_cwd = std::env::current_dir()?;
    std::env::set_current_dir(work.path())?;

    let fixture_queues = scenario.fixture_queues();
    let capture = FixtureCapture::new(scenario_dir.to_path_buf(), fixture_queues);
    let capture_log = Arc::new(Mutex::new(Vec::new()));

    let mut orchestrator = ScreenshotOrchestrator::with_minimum_interval(
        Box::new(LoggingCapture {
            inner: capture,
            log: capture_log.clone(),
        }),
        Duration::from_millis(0),
    );

    let (embed_tx, mut embed_rx) = mpsc::channel::<intime_ai::models::EmbeddingTask>(32);
    let mut fingerprint = None;
    let flags = FeatureFlags::default();
    let mut sessions = SessionTracker::new(flags.clone());
    let rules = CategoryRulesCache::load(&db.storage)
        .await
        .context("load category rules")?;

    for step in &scenario.steps {
        let event = step_to_event(step, &mut fingerprint)?;
        handle_incoming_event(
            Arc::new(event),
            &db.storage,
            &mut orchestrator,
            embed_tx.clone(),
            &flags,
            &mut sessions,
            &rules,
        )
        .await
        .with_context(|| format!("step {:?}", step))?;
    }
    drop(embed_tx);

    let mut service =
        EmbeddingService::new(embedding_server_url).context("embedding server URL")?;
    reqwest::get(format!(
        "{}/",
        embedding_server_url.trim_end_matches('/')
    ))
    .await
    .context("embedding server unreachable")?
    .error_for_status()
    .context("embedding server health")?;

    start_embedding_worker(&mut embed_rx, &mut service, &db.storage)
        .await
        .context("embedding worker")?;

    let events = db.storage.event_repository.list_events(1000).await?;
    let embedding_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM embedding")
        .fetch_one(&db.pool)
        .await?;

    let screenshot_count = capture_log.lock().unwrap().len();
    assert_expectations(&scenario, &db, &events, screenshot_count, embedding_count).await?;

    let report = ReplayReport {
        event_count: events.len(),
        screenshot_count,
        embedding_count,
        database_url: db.database_url.clone(),
        work_dir: work.path().to_path_buf(),
    };

    std::env::set_current_dir(old_cwd)?;
    drop(db);
    drop(work);
    Ok(report)
}

struct LoggingCapture {
    inner: FixtureCapture,
    log: Arc<Mutex<Vec<u64>>>,
}

impl intime_platform::traits::ScreenshotSource for LoggingCapture {
    fn capture(
        &mut self,
        handle: u64,
    ) -> Result<intime_platform::CapturedImage, intime_platform::error::PlatformError> {
        let img = self.inner.capture(handle)?;
        self.log.lock().unwrap().push(handle);
        Ok(img)
    }
}

fn step_to_event(step: &ScenarioStep, fingerprint: &mut Option<blake3::Hash>) -> Result<Event> {
    match step {
        ScenarioStep::AppSeen {
            handle,
            title,
            file_path,
            company,
            product,
            url,
            workspace,
            ..
        } => {
            let details = AppDetails {
                title: title.clone(),
                file_path: file_path.clone(),
                aumid: None,
                company_name: company.clone(),
                product_name: product.clone().or_else(|| Some(title.clone())),
                version_info: None,
                signature_info: None,
            };
            let fp = details.fingerprint();
            *fingerprint = Some(fp);
            Ok(Event {
                timestamp: Timestamp::now(),
                data: EventData::AppSeen {
                    fingerprint: fp,
                    details,
                    window_handle: *handle,
                },
                metadata: EventMetadata {
                    window_title: Some(title.clone()),
                    executable_path: Some(file_path.clone()),
                    url: url.clone(),
                    workspace_path: workspace.clone(),
                    ..Default::default()
                },
            })
        }
        ScenarioStep::WindowFocus {
            handle,
            title,
            focused_element,
            url,
            workspace,
            document_name,
            ..
        } => {
            let fp = fingerprint.context("WindowFocus before AppSeen")?;
            Ok(Event {
                timestamp: Timestamp::now(),
                data: EventData::WindowFocus {
                    fingerprint: fp,
                    window_handle: *handle,
                },
                metadata: EventMetadata {
                    window_title: Some(title.clone()),
                    focused_element: focused_element.clone(),
                    url: url.clone(),
                    workspace_path: workspace.clone(),
                    document_name: document_name.clone(),
                    ..Default::default()
                },
            })
        }
        ScenarioStep::TitleChange {
            handle,
            new_title,
            url,
            workspace,
            ..
        } => {
            let fp = fingerprint.context("TitleChange before AppSeen")?;
            Ok(Event {
                timestamp: Timestamp::now(),
                data: EventData::TitleChange {
                    fingerprint: fp,
                    new_title: new_title.clone(),
                    window_handle: *handle,
                },
                metadata: EventMetadata {
                    window_title: Some(new_title.clone()),
                    url: url.clone(),
                    workspace_path: workspace.clone(),
                    ..Default::default()
                },
            })
        }
        ScenarioStep::IdleStart => Ok(Event {
            timestamp: Timestamp::now(),
            data: EventData::IdleStart,
            metadata: Default::default(),
        }),
        ScenarioStep::IdleEnd => Ok(Event {
            timestamp: Timestamp::now(),
            data: EventData::IdleEnd,
            metadata: Default::default(),
        }),
    }
}

async fn assert_expectations(
    scenario: &Scenario,
    db: &TestDatabase,
    events: &[intime_storage::db::models::EventRecord],
    screenshot_count: usize,
    embedding_count: i64,
) -> Result<()> {
    let expect = &scenario.expect;
    if events.len() < expect.min_events {
        bail!(
            "expected at least {} events, got {}",
            expect.min_events,
            events.len()
        );
    }
    for ty in &expect.event_types {
        if !events.iter().any(|e| e.event_type == *ty) {
            bail!("missing expected event type {ty}");
        }
    }
    if screenshot_count < expect.min_screenshots {
        bail!(
            "expected at least {} screenshots, got {screenshot_count}",
            expect.min_screenshots
        );
    }
    if embedding_count < expect.min_embeddings as i64 {
        bail!(
            "expected at least {} embeddings, got {embedding_count}",
            expect.min_embeddings
        );
    }
    if let Some(company) = &expect.company {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT company_name FROM company WHERE company_name = ? LIMIT 1",
        )
        .bind(company)
        .fetch_optional(&db.pool)
        .await?;
        if found.as_deref() != Some(company.as_str()) {
            bail!("expected company {company} to be registered");
        }
    }

    let sessions: Vec<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT intent, context_key FROM session ORDER BY id")
            .fetch_all(&db.pool)
            .await?;

    if let Some(min_sessions) = expect.min_sessions {
        if sessions.len() < min_sessions {
            bail!(
                "expected at least {min_sessions} sessions, got {} ({sessions:?})",
                sessions.len()
            );
        }
    }
    for intent in &expect.intents {
        if !sessions.iter().any(|(i, _)| i.as_deref() == Some(intent.as_str())) {
            bail!("missing session intent {intent}; sessions={sessions:?}");
        }
    }
    for prefix in &expect.context_key_prefixes {
        if !sessions
            .iter()
            .filter_map(|(_, k)| k.as_deref())
            .any(|k| k.starts_with(prefix.as_str()))
        {
            bail!("missing context_key prefix {prefix}; sessions={sessions:?}");
        }
    }
    Ok(())
}
