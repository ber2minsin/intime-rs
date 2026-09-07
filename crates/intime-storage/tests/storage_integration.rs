use intime_ai::models::{
    EmbeddingRequest, EmbeddingResponse, EmbeddingType, blob_to_vec, vec_to_blob,
};
use intime_core::{
    models::{AppDetails, Event, EventData, EventMetadata, SignatureInfo, VersionInfo},
    time::Timestamp,
};
use intime_storage::testing::TestDatabase;

fn sample_details(name: &str, company: Option<&str>) -> AppDetails {
    AppDetails {
        title: format!("{name} window"),
        file_path: format!("/usr/bin/{name}"),
        aumid: None,
        company_name: company.map(str::to_string),
        product_name: Some(name.to_string()),
        version_info: Some(VersionInfo {
            file_version: Some("1.0.0".into()),
            ..Default::default()
        }),
        signature_info: None,
    }
}

fn focus_event(details: &AppDetails, handle: u64) -> Event {
    Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: details.fingerprint(),
            window_handle: handle,
        },
        metadata: EventMetadata {
            window_title: Some(details.title.clone()),
            executable_path: Some(details.file_path.clone()),
            focused_element: Some("Editor".into()),
            text_changed: false,
            ..Default::default()
        },
    }
}

fn embedding_of(seed: f32) -> Vec<f32> {
    (0..512).map(|i| seed + (i as f32) * 0.001).collect()
}

async fn register_app(db: &TestDatabase, details: &AppDetails) -> i64 {
    let fp = details.fingerprint();
    db.storage
        .app_repository
        .ensure_app(fp, details)
        .await
        .expect("ensure app")
}

#[tokio::test]
async fn creates_migrated_database_with_sqlite_vec() {
    let db = TestDatabase::new().await.expect("test db");

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name",
    )
    .fetch_all(&db.pool)
    .await
    .expect("list tables");

    assert!(tables.iter().any(|t| t == "app"));
    assert!(tables.iter().any(|t| t == "company"));
    assert!(tables.iter().any(|t| t == "event"));
    assert!(tables.iter().any(|t| t == "embedding"));
    assert!(tables.iter().any(|t| t.starts_with("vec_embedding")));

    let distance: f64 = sqlx::query_scalar("SELECT vec_distance_cosine(?, ?)")
        .bind(vec_to_blob(&[1.0, 0.0, 0.0]))
        .bind(vec_to_blob(&[1.0, 0.0, 0.0]))
        .fetch_one(&db.pool)
        .await
        .expect("sqlite-vec distance");
    assert!((distance - 0.0).abs() < 1e-6);
}

#[tokio::test]
async fn registers_company_and_app_once() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("code", Some("Acme"));
    let fp = details.fingerprint();

    let company_id = db
        .storage
        .app_repository
        .add_company(&"Acme".to_string())
        .await
        .expect("add company");
    assert!(company_id > 0);
    assert_eq!(
        db.storage
            .app_repository
            .seen_company(&"Acme".to_string())
            .await
            .expect("seen company"),
        company_id
    );

    assert!(
        !db.storage
            .app_repository
            .seen_app(fp)
            .await
            .expect("seen app before insert")
    );
    db.storage
        .app_repository
        .add_app(fp, &details, Some(company_id))
        .await
        .expect("add app");
    assert!(
        db.storage
            .app_repository
            .seen_app(fp)
            .await
            .expect("seen app after insert")
    );

    let app_id = db
        .storage
        .app_repository
        .get_app_id(fp)
        .await
        .expect("get app id");
    let company = db
        .storage
        .app_repository
        .get_app_company(app_id)
        .await
        .expect("get company");
    assert_eq!(company.company_name, "Acme");
}

#[tokio::test]
async fn persists_event_payload_and_screenshot_path() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("mail", Some("MailCorp"));
    let app_id = register_app(&db, &details).await;

    let event = Event {
        timestamp: Timestamp::now(),
        data: EventData::TitleChange {
            fingerprint: details.fingerprint(),
            new_title: "Writing to Alice".into(),
            window_handle: 42,
        },
        metadata: EventMetadata {
            window_title: Some("Writing to Alice".into()),
            focused_element: Some("Body".into()),
            text_changed: true,
            ..Default::default()
        },
    };
    let screenshot = Some("data/screenshots/42_test.jpg".to_string());
    let event_id = db
        .storage
        .event_repository
        .add_event(&event, Some(app_id), &screenshot, None)
        .await
        .expect("add event");

    let stored = db
        .storage
        .event_repository
        .get_event(event_id)
        .await
        .expect("get event");
    assert_eq!(stored.event_type, "title_change");
    assert_eq!(stored.app_id, Some(app_id));
    assert_eq!(
        stored.screenshot_path.as_deref(),
        Some("data/screenshots/42_test.jpg")
    );

    let payload = stored.payload.expect("payload");
    let decoded: Event = serde_json::from_str(&payload).expect("decode payload");
    assert_eq!(decoded.metadata.focused_element.as_deref(), Some("Body"));
    assert!(decoded.metadata.text_changed);
    assert_eq!(decoded.data.name(), "title_change");
}

#[tokio::test]
async fn stores_and_searches_embeddings_with_sqlite_vec() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("browser", None);
    let app_id = register_app(&db, &details).await;

    let event = focus_event(&details, 7);
    let event_id = db
        .storage
        .event_repository
        .add_event(&event, Some(app_id), &None, None)
        .await
        .unwrap();

    let target = embedding_of(1.0);
    let other = embedding_of(9.0);

    db.storage
        .embedding_repository
        .add_embedding(
            event_id,
            EmbeddingResponse {
                embedding_type: EmbeddingType::Image,
                backend: "test-backend".into(),
                embedding: target.clone(),
            },
        )
        .await
        .expect("add target embedding");

    let event2 = focus_event(&details, 8);
    let event_id2 = db
        .storage
        .event_repository
        .add_event(&event2, Some(app_id), &None, None)
        .await
        .unwrap();
    db.storage
        .embedding_repository
        .add_embedding(
            event_id2,
            EmbeddingResponse {
                embedding_type: EmbeddingType::Image,
                backend: "test-backend".into(),
                embedding: other,
            },
        )
        .await
        .expect("add other embedding");

    let hits = db
        .storage
        .embedding_repository
        .search_embedding(target)
        .await
        .expect("search");
    assert!(!hits.is_empty());
    assert_eq!(hits[0].event_id, event_id);
    assert!(hits[0].similarity >= hits.last().unwrap().similarity);
}

#[tokio::test]
async fn lists_events_in_insertion_order() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("term", None);
    let app_id = register_app(&db, &details).await;

    for handle in [1, 2, 3] {
        let event = focus_event(&details, handle);
        db.storage
            .event_repository
            .add_event(&event, Some(app_id), &None, None)
            .await
            .unwrap();
    }

    let events = db
        .storage
        .event_repository
        .list_events(10)
        .await
        .expect("list");
    assert_eq!(events.len(), 3);
    assert!(events[0].id < events[1].id && events[1].id < events[2].id);
    assert!(events.iter().all(|e| e.event_type == "window_focus"));
}

#[tokio::test]
async fn persists_every_event_variant_with_round_tripped_payload() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("suite", Some("SuiteCo"));
    let app_id = register_app(&db, &details).await;
    let fp = details.fingerprint();

    let variants = vec![
        EventData::WindowFocus {
            fingerprint: fp,
            window_handle: 1,
        },
        EventData::TitleChange {
            fingerprint: fp,
            new_title: "Docs".into(),
            window_handle: 2,
        },
        EventData::TextChanged {
            fingerprint: fp,
            window_handle: 3,
        },
        EventData::UiAction {
            kind: intime_core::models::UiActionKind::FormSubmit,
            fingerprint: fp,
            window_handle: 7,
            label: Some("Submit".into()),
        },
        EventData::IdleStart,
        EventData::IdleEnd,
        EventData::Gap,
        EventData::AppSeen {
            fingerprint: fp,
            details: details.clone(),
            window_handle: 4,
        },
        EventData::Background { fingerprint: fp },
    ];

    let mut ids = Vec::new();
    for data in variants {
        let name = data.name().to_string();
        let event = Event {
            timestamp: Timestamp::now(),
            data,
            metadata: EventMetadata {
                window_title: Some("meta".into()),
                process_id: Some(4242),
                executable_path: Some(details.file_path.clone()),
                focused_element: Some("Main".into()),
                focused_element_class: Some("Edit".into()),
                focused_control_type: Some("Edit".into()),
                automation_id: Some("main.edit".into()),
                text_changed: name == "text_changed",
                ..Default::default()
            },
        };
        let id = db
            .storage
            .event_repository
            .add_event(&event, Some(app_id), &None, None)
            .await
            .unwrap_or_else(|e| panic!("insert {name}: {e}"));
        ids.push((id, name));
    }

    assert_eq!(ids.len(), 9);
    for (id, expected_type) in ids {
        let stored = db.storage.event_repository.get_event(id).await.unwrap();
        assert_eq!(stored.event_type, expected_type);
        let payload = stored.payload.expect("payload");
        let decoded: Event = serde_json::from_str(&payload).unwrap();
        assert_eq!(decoded.data.name(), expected_type);
        assert_eq!(decoded.metadata.process_id, Some(4242));
        assert_eq!(decoded.metadata.automation_id.as_deref(), Some("main.edit"));
    }
}

#[tokio::test]
async fn events_can_exist_without_app_or_screenshot() {
    let db = TestDatabase::new().await.expect("test db");
    let event = Event {
        timestamp: Timestamp::now(),
        data: EventData::IdleStart,
        metadata: Default::default(),
    };
    let id = db
        .storage
        .event_repository
        .add_event(&event, None, &None, None)
        .await
        .expect("orphan idle");
    let stored = db.storage.event_repository.get_event(id).await.unwrap();
    assert!(stored.app_id.is_none());
    assert!(stored.screenshot_path.is_none());
}

#[tokio::test]
async fn get_missing_event_returns_database_error() {
    let db = TestDatabase::new().await.expect("test db");
    let err = db
        .storage
        .event_repository
        .get_event(9_999_999)
        .await
        .expect_err("missing event");
    let msg = format!("{err}");
    assert!(msg.contains("Database") || msg.contains("no rows") || msg.contains("RowNotFound"));
}

#[tokio::test]
async fn list_events_respects_limit() {
    let db = TestDatabase::new().await.expect("test db");
    for _ in 0..5 {
        db.storage
            .event_repository
            .add_event(
                &Event {
                    timestamp: Timestamp::now(),
                    data: EventData::Gap,
                    metadata: Default::default(),
                },
                None,
                &None,
                None,
            )
            .await
            .unwrap();
    }
    let listed = db.storage.event_repository.list_events(2).await.unwrap();
    assert_eq!(listed.len(), 2);
}

#[tokio::test]
async fn company_names_are_unique() {
    let db = TestDatabase::new().await.expect("test db");
    let name = "UniqueCo".to_string();
    db.storage
        .app_repository
        .add_company(&name)
        .await
        .expect("first");
    let err = db
        .storage
        .app_repository
        .add_company(&name)
        .await
        .expect_err("duplicate company");
    assert!(format!("{err}").contains("UNIQUE") || format!("{err}").contains("unique") || format!("{err}").contains("Database"));
}

#[tokio::test]
async fn app_without_company_cannot_resolve_company_row() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("solo", None);
    let app_id = register_app(&db, &details).await;
    let err = db
        .storage
        .app_repository
        .get_app_company(app_id)
        .await
        .expect_err("no company");
    assert!(format!("{err}").contains("Database") || format!("{err}").contains("RowNotFound") || format!("{err}").contains("no rows"));
}

#[tokio::test]
async fn publisher_signature_registers_as_company_source_via_fingerprint() {
    let db = TestDatabase::new().await.expect("test db");
    let details = AppDetails {
        title: "Signed".into(),
        file_path: "/opt/signed".into(),
        aumid: None,
        company_name: None,
        product_name: Some("SignedApp".into()),
        version_info: None,
        signature_info: Some(SignatureInfo {
            publisher: Some("Trusted Publisher".into()),
            subject_full: Some("CN=Trusted Publisher".into()),
            ..Default::default()
        }),
    };
    assert_eq!(details.company().as_deref(), Some("Trusted Publisher"));
    let app_id = register_app(&db, &details).await;
    let company = db
        .storage
        .app_repository
        .get_app_company(app_id)
        .await
        .unwrap();
    assert_eq!(company.company_name, "Trusted Publisher");
}

#[tokio::test]
async fn embedding_blob_helpers_match_stored_vector_length() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("vec", None);
    let app_id = register_app(&db, &details).await;
    let event_id = db
        .storage
        .event_repository
        .add_event(&focus_event(&details, 1), Some(app_id), &None, None)
        .await
        .unwrap();

    let vector = embedding_of(0.5);
    assert_eq!(vector.len(), 512);
    assert_eq!(vec_to_blob(&vector).len(), 512 * 4);
    assert_eq!(blob_to_vec(&vec_to_blob(&vector)), vector);

    db.storage
        .embedding_repository
        .add_embedding(
            event_id,
            EmbeddingResponse {
                embedding_type: EmbeddingType::Text,
                backend: "unit".into(),
                embedding: vector.clone(),
            },
        )
        .await
        .unwrap();

    let hits = db
        .storage
        .embedding_repository
        .search_embedding(vector)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].event_id, event_id);
    assert!(hits[0].similarity > 0.99);
}

#[tokio::test]
async fn isolated_test_databases_do_not_share_state() {
    let a = TestDatabase::new().await.unwrap();
    let b = TestDatabase::new().await.unwrap();
    assert_ne!(a.database_url, b.database_url);

    a.storage
        .event_repository
        .add_event(
            &Event {
                timestamp: Timestamp::now(),
                data: EventData::IdleStart,
                metadata: Default::default(),
            },
            None,
            &None,
            None,
        )
        .await
        .unwrap();

    assert_eq!(a.storage.event_repository.list_events(10).await.unwrap().len(), 1);
    assert!(b.storage.event_repository.list_events(10).await.unwrap().is_empty());
}

#[tokio::test]
async fn ensure_app_normalizes_aumid_version_and_signature() {
    let db = TestDatabase::new().await.expect("test db");
    let details = AppDetails {
        title: "Brave".into(),
        file_path: "/opt/brave.com/brave/brave".into(),
        aumid: Some("brave-browser".into()),
        company_name: Some("Brave Software".into()),
        product_name: Some("brave-browser".into()),
        version_info: Some(VersionInfo {
            file_version: Some("1.2.3".into()),
            original_filename: Some("brave".into()),
            ..Default::default()
        }),
        signature_info: Some(SignatureInfo {
            publisher: Some("Brave Software, Inc.".into()),
            ..Default::default()
        }),
    };
    let app_id = db
        .storage
        .app_repository
        .ensure_app(details.fingerprint(), &details)
        .await
        .unwrap();

    let aumid_id: i64 = sqlx::query_scalar("SELECT aumid_id FROM app WHERE id = ?")
        .bind(app_id)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    let aumid = db.storage.app_repository.get_aumid(aumid_id).await.unwrap();
    assert_eq!(aumid.aumid, "brave-browser");

    let versions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_version_info WHERE app_id = ?")
            .bind(app_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(versions, 1);

    let signatures: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM app_signature WHERE app_id = ?")
            .bind(app_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(signatures, 1);
}

#[tokio::test]
async fn persists_queryable_event_context_columns() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("code", Some("Acme"));
    let app_id = register_app(&db, &details).await;
    let event = Event {
        timestamp: Timestamp::now(),
        data: EventData::WindowFocus {
            fingerprint: details.fingerprint(),
            window_handle: 80,
        },
        metadata: EventMetadata {
            window_title: Some("pipeline.rs — intime-rs — Cursor".into()),
            process_id: Some(4242),
            executable_path: Some(details.file_path.clone()),
            focused_element: Some("Editor".into()),
            document_name: Some("pipeline.rs".into()),
            workspace_path: Some("intime-rs".into()),
            ..Default::default()
        },
    };
    let id = db
        .storage
        .event_repository
        .add_event(&event, Some(app_id), &None, None)
        .await
        .unwrap();
    let stored = db.storage.event_repository.get_event(id).await.unwrap();
    assert_eq!(stored.window_handle, Some(80));
    assert_eq!(stored.process_id, Some(4242));
    assert_eq!(stored.document_name.as_deref(), Some("pipeline.rs"));
    assert_eq!(stored.workspace_path.as_deref(), Some("intime-rs"));
    assert_eq!(stored.app_id, Some(app_id));
    let decoded: Event = serde_json::from_str(stored.payload.as_deref().unwrap()).unwrap();
    assert_eq!(decoded.metadata.focused_element.as_deref(), Some("Editor"));
}

#[tokio::test]
async fn session_and_rules_round_trip() {
    let db = TestDatabase::new().await.expect("test db");
    let cats = db
        .storage
        .session_repository
        .list_categories()
        .await
        .unwrap();
    assert!(cats.iter().any(|c| c.slug == "code_editing"));
    let coding_id = cats.iter().find(|c| c.slug == "code_editing").unwrap().id;
    let rules = db
        .storage
        .session_repository
        .list_activity_rules()
        .await
        .unwrap();
    assert!(!rules.is_empty());

    let started = Timestamp::now().as_datetime();
    let details = sample_details("code", None);
    let app_id = register_app(&db, &details).await;
    let session_id = db
        .storage
        .session_repository
        .open_session(
            started,
            Some("code_editing"),
            Some("Editing pipeline"),
            "heuristic",
            Some(coding_id),
            Some("app:code|doc:pipeline.rs"),
            Some(app_id),
        )
        .await
        .unwrap();

    let event_id = db
        .storage
        .event_repository
        .add_event(
            &focus_event(&details, 9),
            Some(app_id),
            &None,
            Some(session_id),
        )
        .await
        .unwrap();
    let stored = db.storage.event_repository.get_event(event_id).await.unwrap();
    assert_eq!(stored.session_id, Some(session_id));
    assert_eq!(stored.window_handle, Some(9));
    assert!(stored.window_title.is_some());

    let session = db
        .storage
        .session_repository
        .get_session(session_id)
        .await
        .unwrap();
    assert_eq!(session.category_id, Some(coding_id));
    assert_eq!(session.intent.as_deref(), Some("code_editing"));
    assert_eq!(session.context_key.as_deref(), Some("app:code|doc:pipeline.rs"));
    assert_eq!(session.app_id, Some(app_id));
    assert_eq!(session.important, 0);

    db.storage
        .session_repository
        .set_session_important(session_id, true)
        .await
        .unwrap();
    let session = db
        .storage
        .session_repository
        .get_session(session_id)
        .await
        .unwrap();
    assert_eq!(session.important, 1);
}

#[tokio::test]
async fn screenshot_tier_and_clear_round_trip() {
    let db = TestDatabase::new().await.expect("test db");
    let details = sample_details("code", None);
    let app_id = register_app(&db, &details).await;
    let path = Some("data/screenshots/test.jpg".to_string());
    let id = db
        .storage
        .event_repository
        .add_event(&focus_event(&details, 3), Some(app_id), &path, None)
        .await
        .unwrap();
    let stored = db.storage.event_repository.get_event(id).await.unwrap();
    assert_eq!(stored.screenshot_path.as_deref(), Some("data/screenshots/test.jpg"));
    assert_eq!(stored.screenshot_tier.as_deref(), Some("full"));

    db.storage
        .event_repository
        .set_screenshot_tier(id, "compact", None)
        .await
        .unwrap();
    let stored = db.storage.event_repository.get_event(id).await.unwrap();
    assert_eq!(stored.screenshot_tier.as_deref(), Some("compact"));
    assert!(stored.screenshot_path.is_some());

    db.storage
        .event_repository
        .clear_screenshot(id)
        .await
        .unwrap();
    let stored = db.storage.event_repository.get_event(id).await.unwrap();
    assert!(stored.screenshot_path.is_none());
    assert!(stored.screenshot_tier.is_none());

    let candidates = db
        .storage
        .event_repository
        .list_screenshot_candidates(10)
        .await
        .unwrap();
    assert!(candidates.iter().all(|c| c.event_id != id));
}

#[tokio::test]
async fn request_response_types_are_json_compatible_with_storage_payloads() {
    let req = EmbeddingRequest::Image {
        image_path: "data/screenshots/1.jpg".into(),
    };
    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["type"], "image");

    let resp = EmbeddingResponse {
        embedding_type: EmbeddingType::Image,
        backend: "clip".into(),
        embedding: vec![0.0; 512],
    };
    let encoded = serde_json::to_string(&resp.embedding_type).unwrap();
    assert!(encoded.contains("image"));
}
