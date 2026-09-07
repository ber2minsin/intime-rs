use sqlx::SqlitePool;

pub mod app;
pub mod embedding;
pub mod event;
pub mod models;
pub mod pool;
pub mod session;

#[derive(Clone)]
pub struct SqliteRepository {
    pub pool: SqlitePool,
}

impl SqliteRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
