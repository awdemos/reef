use thiserror::Error;

pub type EngineResult<T> = Result<T, EngineError>;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum EngineError {
    #[error("invalid state transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },

    #[error("run not found: {0}")]
    RunNotFound(String),

    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("rag error: {0}")]
    Rag(String),

    #[error("cancelled")]
    Cancelled,

    #[error("timeout")]
    Timeout,

    #[error("internal error: {0}")]
    Internal(String),
}
