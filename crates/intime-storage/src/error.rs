use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    // Add this line
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{entity} is not found on the Storage")]
    NotFound { entity: &'static str },
}
