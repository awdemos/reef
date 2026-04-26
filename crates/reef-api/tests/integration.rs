use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use reef_api::{build_router, AppState, ServerConfig};
use reef_engine::{CapabilityRegistry, RunWorker, RunWorkerConfig, QueueBackend};
use reef_storage::{MemoryBackend, Principal, Scope};
use tower::ServiceExt;

async fn test_auth_middleware(mut request: axum::extract::Request, next: Next) -> Response {
    let principal = Principal {
        user_id: "test-user".into(),
        tenant_id: None,
        scopes: vec![Scope::Read, Scope::Write, Scope::FileSearch, Scope::ApiKeyManage],
    };
    request.extensions_mut().insert(principal);
    next.run(request).await
}

fn setup_app() -> axum::Router {
    let storage = Arc::new(MemoryBackend::new());
    let registry = Arc::new(CapabilityRegistry::new(
        std::time::Duration::from_secs(10),
        std::time::Duration::from_secs(30),
    ));
    let worker = Arc::new(RunWorker::new(
        storage.clone(),
        RunWorkerConfig::default(),
        QueueBackend::Memory,
    ));

    let state = Arc::new(AppState {
        storage,
        registry,
        worker,
        config: ServerConfig {
            enable_dev_routes: true,
            ..Default::default()
        },
    });

    build_router(state).layer(middleware::from_fn(test_auth_middleware))
}

#[tokio::test]
async fn test_health() {
    let app = setup_app();
    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_thread() {
    let app = setup_app();
    let body = serde_json::json!({
        "metadata": {"test": true}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/threads")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_list_threads() {
    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/threads")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_assistant() {
    let app = setup_app();
    let body = serde_json::json!({
        "model": "gpt-4",
        "name": "Test Assistant",
        "instructions": "Be helpful"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/assistants")
                .header("content-type", "application/json")
                .header("authorization", "Bearer test-token")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_dev_models() {
    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/dev/models")
                .header("authorization", "Bearer test-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_metrics() {
    let app = setup_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
