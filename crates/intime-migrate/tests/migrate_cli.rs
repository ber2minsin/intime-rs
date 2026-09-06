use intime_ai::models::vec_to_blob;
use intime_migrate::run_migrations;

#[tokio::test]
async fn migrate_creates_schema_and_loads_sqlite_vec() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("migrate-test.db");
    let url = format!("sqlite://{}?mode=rwc", path.display());

    run_migrations(&url).await.expect("first migrate");
    // Idempotent: second run should succeed.
    run_migrations(&url).await.expect("second migrate");

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("connect");

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("tables");

    for required in ["app", "company", "event", "embedding"] {
        assert!(
            tables.iter().any(|t| t == required),
            "missing table {required}: {tables:?}"
        );
    }
    assert!(tables.iter().any(|t| t.starts_with("vec_embedding")));

    let distance: f64 = sqlx::query_scalar("SELECT vec_distance_cosine(?, ?)")
        .bind(vec_to_blob(&[0.0, 1.0, 0.0]))
        .bind(vec_to_blob(&[0.0, 1.0, 0.0]))
        .fetch_one(&pool)
        .await
        .expect("sqlite-vec");
    assert!((distance - 0.0).abs() < 1e-6);

    pool.close().await;
}
