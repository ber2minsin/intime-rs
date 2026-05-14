use sqlx::SqlitePool;


pub mod app;
pub mod embedding;
pub mod event;
pub mod models;
pub mod pool;

#[derive(Clone)]
pub struct SqliteRepository {
    pub pool: SqlitePool,
}

impl SqliteRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
