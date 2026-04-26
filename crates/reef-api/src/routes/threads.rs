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
    StorageBackend,
};
use uuid::Uuid;

use crate::{
    error::{ApiError, ApiResult},
    middleware::AuthPrincipal,
    models::*,
    state::AppState,
};

// ------------------------------------------------------------------
// Threads
// ------------------------------------------------------------------

pub async fn create_thread(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Json(req): Json<CreateThreadRequest>,
) -> ApiResult<Json<ThreadObject>> {
    let thread = Thread {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        title: None,
        metadata: req.metadata.unwrap_or(serde_json::Value::Null),
    };

    let created = state.storage.create_thread(&principal, &thread).await?;
    Ok(Json(created.into()))
}

pub async fn get_thread(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
) -> ApiResult<Json<ThreadObject>> {
    let thread = state.storage.get_thread(&principal, thread_id).await?;
    Ok(Json(thread.into()))
}

pub async fn update_thread(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
    Json(req): Json<CreateThreadRequest>,
) -> ApiResult<Json<ThreadObject>> {
    let mut thread = state.storage.get_thread(&principal, thread_id).await?;
    if let Some(metadata) = req.metadata {
        thread.metadata = metadata;
    }
    thread.updated_at = Utc::now();

    let updated = state.storage.update_thread(&principal, &thread).await?;
    Ok(Json(updated.into()))
}

pub async fn delete_thread(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.storage.delete_thread(&principal, thread_id).await?;
    Ok(StatusCode::OK)
}

pub async fn list_threads(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
) -> ApiResult<Json<ListResponse<ThreadObject>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Desc,
        ..Default::default()
    };
    let page = state.storage.list_threads(&principal, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}

// ------------------------------------------------------------------
// Messages
// ------------------------------------------------------------------

pub async fn create_message(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
    Json(req): Json<CreateMessageRequest>,
) -> ApiResult<Json<MessageObject>> {
    let content = vec![MessageContent::Text { text: req.content }];

    let message = Message {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thread_id,
        run_id: None,
        role: req.role,
        content,
        annotations: vec![],
        metadata: req.metadata.unwrap_or(serde_json::Value::Null),
    };

    let created = state.storage.create_message(&principal, &message).await?;
    Ok(Json(created.into()))
}

pub async fn list_messages(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
) -> ApiResult<Json<ListResponse<MessageObject>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Asc,
        ..Default::default()
    };
    let page = state.storage.list_messages(&principal, thread_id, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}

// ------------------------------------------------------------------
// Runs
// ------------------------------------------------------------------

pub async fn create_run(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
    Json(req): Json<CreateRunRequest>,
) -> ApiResult<Json<RunObject>> {
    let run = Run {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        thread_id,
        assistant_id: req.assistant_id.and_then(|s| Uuid::parse_str(&s).ok()),
        status: RunStatus::Queued,
        model: req.model.unwrap_or_else(|| "default".into()),
        instructions: req.instructions,
        tools: req.tools.unwrap_or_default(),
        temperature: req.temperature,
        max_prompt_tokens: req.max_prompt_tokens,
        max_completion_tokens: req.max_completion_tokens,
        metadata: req.metadata.unwrap_or(serde_json::Value::Null),
        started_at: None,
        completed_at: None,
        expired_at: None,
        failed_at: None,
        last_error: None,
    };

    let created = state.storage.create_run(&principal, &run).await?;

    // Enqueue for worker processing
    state.worker.enqueue(created.id).await.map_err(|e| {
        ApiError::Internal
    })?;

    Ok(Json(created.into()))
}

pub async fn get_run(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path((thread_id, run_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<RunObject>> {
    let run = state.storage.get_run(&principal, run_id).await?;
    Ok(Json(run.into()))
}

pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(thread_id): Path<Uuid>,
) -> ApiResult<Json<ListResponse<RunObject>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Desc,
        ..Default::default()
    };
    let page = state.storage.list_runs(&principal, thread_id, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}

pub async fn cancel_run(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path((thread_id, run_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<Json<RunObject>> {
    state.worker.cancel(run_id).await.map_err(|_| ApiError::Internal)?;
    let run = state.storage.get_run(&principal, run_id).await?;
    Ok(Json(run.into()))
}
