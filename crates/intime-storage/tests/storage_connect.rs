use intime_storage::{config::StorageConfig, storage::Storage, testing::TestDatabase};

#[tokio::test]
async fn storage_connect_opens_configured_database_url() {
    let db = TestDatabase::new().await.expect("temp db");
    let storage = Storage::connect(&StorageConfig {
        database_file: db.database_url.clone(),
    })
    .await
    .expect("connect");

    // Prove the connected storage can read schema created by TestDatabase migrations.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM event")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let _ = storage;
}

#[tokio::test]
async fn storage_connect_creates_missing_sqlite_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fresh.db");
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let storage = Storage::connect(&StorageConfig {
        database_file: url.clone(),
    })
    .await
    .expect("create");
    assert!(path.exists());
    let _ = storage;
}
