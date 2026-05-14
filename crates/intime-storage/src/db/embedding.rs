use async_trait::async_trait;
use intime_ai::models::{EmbeddingResponse, vec_to_blob};

use crate::{
    db::{SqliteRepository, models::EmbeddingRecord},
    error::StorageError,
    repository::embedding::EmbeddingRepository,
};

#[async_trait]
impl EmbeddingRepository for SqliteRepository {
    async fn add_embedding(
        &self,
        event_id: i64,
        embedding_response: EmbeddingResponse,
    ) -> Result<(), StorageError> {
        let embedding_type = serde_json::to_string(&embedding_response.embedding_type)?;
        let embedding_id = sqlx::query_scalar!(
            "INSERT INTO embedding (event_id, embedding_type, embedding_backend) VALUES (?, ?, ?) RETURNING id",
            event_id,
            embedding_type,
            embedding_response.backend
        ).fetch_one(&self.pool).await?;

        // Virtual table backed by extension — use runtime query, no !
        let embedding_float = vec_to_blob(&embedding_response.embedding);
        sqlx::query("INSERT INTO vec_embedding (id, embedding_data) VALUES (?, ?)")
            .bind(embedding_id)
            .bind(embedding_float)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn search_embedding(
        &self,
        embedding: Vec<f32>,
    ) -> Result<Vec<EmbeddingRecord>, StorageError> {
        let embedding_blob = vec_to_blob(&embedding);
        let limit: i64 = 5;

        let records = sqlx::query_as::<_, EmbeddingRecord>(
            "SELECT 
            e.id, 
            e.event_id, 
            e.embedding_type, 
            e.embedding_backend, 
            v.embedding_data,
            (1 - vec_distance_cosine(v.embedding_data, $1)) AS similarity
         FROM embedding e
         JOIN vec_embedding v ON e.id = v.id
         ORDER BY similarity DESC
         LIMIT $2",
        )
        .bind(embedding_blob)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Database(e))?;

        Ok(records)
    }
}
