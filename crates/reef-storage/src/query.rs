use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::ChunkContentType;

/// Specification for paginated, filtered queries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QuerySpec {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub order_by: Option<String>,
    pub direction: Direction,
    pub filters: Vec<Filter>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Direction {
    #[default]
    Desc,
    Asc,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Filter {
    pub field: String,
    pub op: FilterOp,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    In,
    IsNull,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Vector similarity search query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorQuery {
    pub vector_store_id: Uuid,
    pub embedding: Vec<f32>,
    pub top_k: usize,
    pub filter: Option<serde_json::Value>,
    pub min_score: Option<f32>,
}

/// A scored result from vector search.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredChunk {
    pub chunk_id: Uuid,
    pub vector_store_id: Uuid,
    pub file_id: Uuid,
    pub score: f32,
    pub content: String,
    pub content_type: ChunkContentType,
    pub metadata: serde_json::Value,
    pub index: usize,
}
