use libsqlite3_sys::sqlite3_auto_extension;
use sqlite_vec::sqlite3_vec_init;
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().expect("Error loading dot environment");
    let db_url = std::env::var("DATABASE_FILE")
        .expect("DATABASE_FILE must be set");

    // Register before any connection is opened
    unsafe {
        sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite3_vec_init as *const (),
        )));
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await?;

    // sqlx::migrate! only reads SQL files at compile time — no live DB needed
    sqlx::migrate!("../intime-storage/migrations")

        .run(&pool)
        .await?;

    pool.close().await;
    println!("Migrations complete.");
    Ok(())
}