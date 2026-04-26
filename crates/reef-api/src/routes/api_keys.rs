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

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub scopes: Option<Vec<String>>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CreateApiKeyResponse {
    pub id: String,
    pub object: String,
    pub name: String,
    pub key: String,
    pub created_at: i64,
}

pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Json(req): Json<CreateApiKeyRequest>,
) -> ApiResult<Json<CreateApiKeyResponse>> {
    let raw_key = format!("reef_{}", Uuid::new_v4().to_string().replace("-", ""));
    let key_hash = format!("{:x}", md5::compute(&raw_key));
    let key_preview = format!("{}...{}", &raw_key[..4], &raw_key[raw_key.len()-4..]);

    let scopes = req.scopes.map(|s| {
        s.into_iter().filter_map(|scope| match scope.as_str() {
            "read" => Some(Scope::Read),
            "write" => Some(Scope::Write),
            "admin" => Some(Scope::Admin),
            "file_search" => Some(Scope::FileSearch),
            "api_key_manage" => Some(Scope::ApiKeyManage),
            _ => None,
        }).collect()
    }).unwrap_or_else(|| vec![Scope::Read, Scope::Write]);

    let api_key = ApiKey {
        id: Uuid::new_v4(),
        owner_id: principal.user_id.clone(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        key_hash,
        key_preview,
        name: req.name,
        scopes,
        expires_at: req.expires_at.map(|ts| chrono::DateTime::from_timestamp(ts, 0).unwrap_or_else(|| Utc::now())),
        last_used_at: None,
    };

    state.storage.create_api_key(&principal, &api_key).await?;

    Ok(Json(CreateApiKeyResponse {
        id: api_key.id.to_string(),
        object: "api_key".into(),
        name: api_key.name,
        key: raw_key,
        created_at: api_key.created_at.timestamp(),
    }))
}

pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
) -> ApiResult<Json<ListResponse<ApiKeyObject>>> {
    let spec = QuerySpec {
        limit: Some(20),
        direction: Direction::Desc,
        ..Default::default()
    };
    let page = state.storage.list_api_keys(&principal, &spec).await?;

    Ok(Json(ListResponse {
        object: "list".into(),
        data: page.items.into_iter().map(Into::into).collect(),
        first_id: None,
        last_id: None,
        has_more: page.total > page.offset + page.limit,
    }))
}

pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Path(key_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    state.storage.delete_api_key(&principal, key_id).await?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiKeyObject {
    pub id: String,
    pub object: String,
    pub name: String,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

impl From<ApiKey> for ApiKeyObject {
    fn from(k: ApiKey) -> Self {
        Self {
            id: k.id.to_string(),
            object: "api_key".into(),
            name: k.name,
            created_at: k.created_at.timestamp(),
            expires_at: k.expires_at.map(|t| t.timestamp()),
        }
    }
}
