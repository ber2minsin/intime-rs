use anyhow::{Context, Result};
use libsqlite3_sys::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use sqlx::sqlite::SqlitePoolOptions;

/// Register sqlite-vec and apply all storage migrations against `database_url`.
pub async fn run_migrations(database_url: &str) -> Result<()> {
    // Register before any connection is opened.
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .context("failed to connect to database")?;

    sqlx::migrate!("../intime-storage/migrations")
        .run(&pool)
        .await
        .context("migration failed")?;

    pool.close().await;
    Ok(())
}
