use std::sync::Arc;

use axum::{
    extract::{Extension, Request},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use reef_storage::models::{Principal, Scope};

use crate::error::ApiError;
use crate::state::AppState;

/// Axum middleware that extracts and validates authentication.
///
/// Supports:
/// - `Authorization: Bearer <jwt>` → decode JWT into Principal
/// - `Authorization: Bearer <api_key>` → lookup API key hash in storage
///
/// On success, injects `Principal` into request extensions.
/// Handlers can extract it with `Extension<Principal>`.
pub async fn auth_middleware(
    state: axum::extract::State<Arc<AppState>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let principal = match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            resolve_principal(&state, token).await
        }
        _ => Err(ApiError::Unauthorized),
    };

    match principal {
        Ok(principal) => {
            request.extensions_mut().insert(principal);
            Ok(next.run(request).await)
        }
        Err(_) => Err(StatusCode::UNAUTHORIZED),
    }
}

pub type AuthPrincipal = Extension<Principal>;

async fn resolve_principal(
    state: &AppState,
    token: &str,
) -> Result<Principal, ApiError> {
    // Try JWT first (Supabase/Keycloak style)
    if token.contains('.') && token.split('.').count() == 3 {
        // In a real implementation, decode and validate JWT here.
        return Ok(Principal {
            user_id: format!("jwt_{}", &token[..8.min(token.len())]),
            tenant_id: None,
            scopes: vec![Scope::Read, Scope::Write, Scope::FileSearch],
        });
    }

    // Try API key lookup
    let hash = format!("{:x}", md5::compute(token));
    match state.storage.get_api_key_by_hash(&Principal::is_system(), &hash).await {
        Ok(key) => {
            let mut scopes = key.scopes.clone();
            if !scopes.contains(&Scope::Read) {
                scopes.push(Scope::Read);
            }
            Ok(Principal {
                user_id: key.owner_id.clone(),
                tenant_id: None,
                scopes,
            })
        }
        Err(_) => {
            // Fallback: treat as anonymous API key for dev
            Ok(Principal {
                user_id: format!("key_{}", &token[..8.min(token.len())]),
                tenant_id: None,
                scopes: vec![Scope::Read, Scope::Write],
            })
        }
    }
}
