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

pub async fn create_assistant(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Json(req): Json<CreateAssistantRequest>,
) -> ApiResult<Json<AssistantObject>> {
    let assistant = Assistant {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        name: req.name,
        description: req.description,
        model: req.model,
        instructions: req.instructions,
        tools: req.tools.unwrap_or_default(),
        tool_resources: req.tool_resources.unwrap_or(serde_json::Value::Null),
        metadata: req.metadata.unwrap_or(serde_json::Value::Null),
        temperature: req.temperature,
        top_p: req.top_p,
        response_format: req.response_format,
    };

    let created = state.storage.create_assistant(&principal, &assistant).await?;
    Ok(Json(created.into()))
}

pub async fn get_assistant(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(assistant_id): Path<Uuid>,
) -> ApiResult<Json<AssistantObject>> {
    let assistant = state.storage.get_assistant(&principal, assistant_id).await?;
    Ok(Json(assistant.into()))
}

pub async fn update_assistant(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(assistant_id): Path<Uuid>,
    Json(req): Json<CreateAssistantRequest>,
) -> ApiResult<Json<AssistantObject>> {
    let mut assistant = state.storage.get_assistant(&principal, assistant_id).await?;

    if let Some(name) = req.name {
        assistant.name = Some(name);
    }
    if let Some(description) = req.description {
        assistant.description = Some(description);
    }
    assistant.model = req.model;
    if let Some(instructions) = req.instructions {
        assistant.instructions = Some(instructions);
    }
    if let Some(tools) = req.tools {
        assistant.tools = tools;
    }
    if let Some(metadata) = req.metadata {
        assistant.metadata = metadata;
    }
    assistant.updated_at = Utc::now();

    let updated = state.storage.update_assistant(&principal, &assistant).await?;
    Ok(Json(updated.into()))
}

pub async fn delete_assistant(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(assistant_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.storage.delete_assistant(&principal, assistant_id).await?;
    Ok(StatusCode::OK)
}

pub async fn list_assistants(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
) -> ApiResult<Json<ListResponse<AssistantObject>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Desc,
        ..Default::default()
    };
    let page = state.storage.list_assistants(&principal, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}
