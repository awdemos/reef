use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use reef_storage::{
    models::*,
    query::{Direction, QuerySpec},
};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    middleware::AuthPrincipal,
    models::*,
    state::AppState,
};

pub async fn create_vector_store(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Json(req): Json<serde_json::Value>,
) -> ApiResult<Json<VectorStoreObject>> {
    let store = VectorStore {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        name: req.get("name").and_then(|v| v.as_str().map(String::from)),
        bytes: 0,
        file_counts: FileCounts {
            in_progress: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            total: 0,
        },
        status: VectorStoreStatus::InProgress,
        expires_after: req.get("expires_after").cloned(),
        metadata: req.get("metadata").cloned().unwrap_or(serde_json::Value::Null),
    };

    let created = state.storage.create_vector_store(&principal, &store).await?;
    Ok(Json(created.into()))
}

pub async fn get_vector_store(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(vector_store_id): Path<Uuid>,
) -> ApiResult<Json<VectorStoreObject>> {
    let store = state.storage.get_vector_store(&principal, vector_store_id).await?;
    Ok(Json(store.into()))
}

pub async fn delete_vector_store(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(vector_store_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.storage.delete_vector_store(&principal, vector_store_id).await?;
    Ok(StatusCode::OK)
}

pub async fn list_vector_stores(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
) -> ApiResult<Json<ListResponse<VectorStoreObject>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Desc,
        ..Default::default()
    };
    let page = state.storage.list_vector_stores(&principal, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}
