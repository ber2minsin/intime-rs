use std::path::PathBuf;

fn embedding_url() -> Option<String> {
    std::env::var("EMBEDDING_SERVER_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
}

fn scenarios_root() -> PathBuf {
    // Prefer CARGO_MANIFEST_DIR/../../scenarios when running from crates/intime-e2e
    let from_crate = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
    if from_crate.exists() {
        return from_crate.canonicalize().unwrap_or(from_crate);
    }
    PathBuf::from("scenarios")
}

#[tokio::test]
async fn mail_compose_scenario_against_staged_embedding_server() {
    let url = match embedding_url() {
        Some(url) => url,
        None if std::env::var_os("INTIME_E2E_REQUIRED").is_some() => {
            panic!("EMBEDDING_SERVER_URL required when INTIME_E2E_REQUIRED is set");
        }
        None => {
            eprintln!(
                "skipping staged e2e: set EMBEDDING_SERVER_URL (e.g. http://127.0.0.1:8000 after `docker compose up -d embed-stub`)"
            );
            return;
        }
    };

    let dir = scenarios_root().join("mail_compose");
    let report = intime_e2e::replay_scenario(&dir, &url)
        .await
        .expect("mail_compose scenario");

    assert!(report.event_count >= 4);
    assert!(report.screenshot_count >= 2);
    assert!(report.embedding_count >= 2);
}
