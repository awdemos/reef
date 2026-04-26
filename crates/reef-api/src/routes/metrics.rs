use std::sync::Arc;

use axum::{extract::State, Json};

use crate::state::AppState;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsResponse {
    pub uptime_seconds: u64,
    pub registered_models: usize,
    pub healthy_backends: usize,
    pub total_backends: usize,
}

pub async fn metrics(State(state): State<Arc<AppState>>) -> Json<MetricsResponse> {
    let caps = state.registry.list();
    let total = caps.len();
    let healthy = caps.iter().filter(|c| {
        state.registry.get_health(&c.model_id)
            .map(|h| matches!(h.status, reef_engine::registry::HealthStatus::Healthy))
            .unwrap_or(false)
    }).count();

    Json(MetricsResponse {
        uptime_seconds: 0, // Would track actual uptime in production
        registered_models: total,
        healthy_backends: healthy,
        total_backends: total,
    })
}
