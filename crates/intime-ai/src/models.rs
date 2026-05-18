use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingType {
    Text,
    Image,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EmbeddingRequest {
    Text { text: String },
    Image { image_path: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub embedding_type: EmbeddingType,
    pub backend: String,
    pub embedding: Vec<f32>,
}

// Convert Vec<f32> <-> raw little-endian bytes for sqlite-vec
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

pub fn blob_to_vec(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}