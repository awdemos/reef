use thiserror::Error;

pub type RagResult<T> = Result<T, RagError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum RagError {
    #[error("parse failed: {0}")]
    Parse(String),

    #[error("chunk failed: {0}")]
    Chunk(String),

    #[error("embedding failed: {0}")]
    Embedding(String),

    #[error("index failed: {0}")]
    Index(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("internal error: {0}")]
    Internal(String),
}
