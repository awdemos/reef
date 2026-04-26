use std::sync::Arc;

use axum::{extract::State, Json};

use crate::{middleware::AuthPrincipal, state::AppState};

pub async fn list_models(
    State(state): State<Arc<AppState>>,
    _principal: AuthPrincipal,
) -> Json<serde_json::Value> {
    let caps = state.registry.list();
    let models: Vec<serde_json::Value> = caps
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "id": c.model_id,
                "object": "model",
                "provider_url": c.provider_url,
                "modalities": c.modalities,
                "max_context": c.max_context,
                "supports_streaming": c.supports_streaming,
                "hardware": c.hardware_profile,
            })
        })
        .collect();

    Json(serde_json::json!({
        "object": "list",
        "data": models,
    }))
}
