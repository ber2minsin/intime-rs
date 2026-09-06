use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let db_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;
    intime_migrate::run_migrations(&db_url).await?;
    println!("Migrations complete.");
    Ok(())
}
