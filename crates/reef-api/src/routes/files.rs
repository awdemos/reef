use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use reef_storage::{
    models::*,
    query::{Direction, QuerySpec},
};
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    middleware::AuthPrincipal,
    models::*,
    state::AppState,
};

pub async fn upload_file(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    mut multipart: Multipart,
) -> ApiResult<Json<FileObjectApi>> {
    let mut filename = String::new();
    let mut purpose = String::from("assistants");
    let mut bytes: Vec<u8> = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::BadRequest(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        let data = field.bytes().await.map_err(|e| ApiError::BadRequest(e.to_string()))?;

        match name.as_str() {
            "file" => {
                bytes = data.to_vec();
            }
            "purpose" => {
                purpose = String::from_utf8_lossy(&data).to_string();
            }
            "filename" => {
                filename = String::from_utf8_lossy(&data).to_string();
            }
            _ => {}
        }
    }

    if filename.is_empty() {
        filename = "upload.bin".to_string();
    }

    let file = FileObject {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        bytes: bytes.len() as i64,
        filename,
        purpose,
        status: FileStatus::Uploaded,
        status_details: None,
        metadata: serde_json::Value::Null,
    };

    let created = state.storage.create_file(&principal, &file).await?;
    Ok(Json(created.into()))
}

pub async fn get_file(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(file_id): Path<Uuid>,
) -> ApiResult<Json<FileObjectApi>> {
    let file = state.storage.get_file(&principal, file_id).await?;
    Ok(Json(file.into()))
}

pub async fn delete_file(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(file_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.storage.delete_file(&principal, file_id).await?;
    Ok(StatusCode::OK)
}

pub async fn list_files(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
) -> ApiResult<Json<ListResponse<FileObjectApi>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Desc,
        ..Default::default()
    };
    let page = state.storage.list_files(&principal, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}
