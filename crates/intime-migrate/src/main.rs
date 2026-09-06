use anyhow::{Context, Result};
use libsqlite3_sys::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    // Register before any connection is opened
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(sqlite3_vec_init as *const ())));
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .context("failed to connect to database")?;

    sqlx::migrate!("../intime-storage/migrations")
        .run(&pool)
        .await
        .context("migration failed")?;

    pool.close().await;
    println!("Migrations complete.");
    Ok(())
}
