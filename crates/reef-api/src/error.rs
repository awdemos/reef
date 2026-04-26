use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use reef_storage::error::StorageError;
use serde_json::json;
use thiserror::Error;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("internal server error")]
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, self.to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, self.to_string()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            ApiError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            ApiError::Storage(StorageError::NotFound(msg)) => {
                (StatusCode::NOT_FOUND, msg.clone())
            }
            ApiError::Storage(StorageError::Unauthorized(msg)) => {
                (StatusCode::FORBIDDEN, msg.clone())
            }
            ApiError::Storage(StorageError::Conflict(msg)) => {
                (StatusCode::CONFLICT, msg.clone())
            }
            ApiError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            ApiError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "error": {
                "message": message,
                "type": "api_error",
                "code": status.as_u16(),
            }
        }));

        (status, body).into_response()
    }
}
