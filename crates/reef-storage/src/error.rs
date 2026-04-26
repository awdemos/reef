use thiserror::Error;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum StorageError {
    #[error("entity not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("invalid query: {0}")]
    InvalidQuery(String),

    #[error("connection failed: {0}")]
    Connection(String),

    #[error("transaction failed: {0}")]
    Transaction(String),

    #[error("vector operation failed: {0}")]
    Vector(String),

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl StorageError {
    pub fn not_found(entity: impl Into<String>) -> Self {
        Self::NotFound(entity.into())
    }

    pub fn unauthorized(reason: impl Into<String>) -> Self {
        Self::Unauthorized(reason.into())
    }

    pub fn not_implemented(feature: impl Into<String>) -> Self {
        Self::NotImplemented(feature.into())
    }
}
