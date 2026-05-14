use async_trait::async_trait;
use intime_ai::models::EmbeddingResponse;

use crate::{db::models::EmbeddingRecord, error::StorageError};

#[async_trait]
pub trait EmbeddingRepository: Send + Sync {
    async fn add_embedding(
        &self,
        event_id: i64,
        embedding_response: EmbeddingResponse,
    ) -> Result<(), StorageError>;
    async fn search_embedding(
        &self,
        embedding: Vec<f32>,
    ) -> Result<Vec<EmbeddingRecord>, StorageError>;
}
