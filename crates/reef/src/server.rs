use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
};
use reef_api::{build_router, AppState, ServerConfig};
use reef_engine::{CapabilityRegistry, RunWorker, RunWorkerConfig, QueueBackend};
use reef_storage::MemoryBackend;
use tokio::signal;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

pub async fn serve(bind: String, storage: String, dev: bool) -> anyhow::Result<()> {
    // 1. Initialize storage backend
    let storage: Arc<dyn reef_storage::StorageBackend> = match storage.as_str() {
        "memory" => {
            info!("using in-memory storage backend");
            Arc::new(MemoryBackend::new())
        }
        "postgres" => {
            let database_url = std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL required for postgres backend"))?;
            let pool = sqlx::PgPool::connect(&database_url).await?;
            let backend = reef_storage::PostgresBackend::new(pool);
            backend.migrate().await?;
            info!("using postgres storage backend");
            Arc::new(backend)
        }
        "turso" => {
            let turso_url = std::env::var("TURSO_URL")
                .map_err(|_| anyhow::anyhow!("TURSO_URL required for turso backend"))?;
            let turso_token = std::env::var("TURSO_AUTH_TOKEN").ok();
            let backend = reef_storage::TursoBackend::connect(&turso_url, turso_token.as_deref()).await
                .map_err(|e| anyhow::anyhow!("failed to connect to turso: {}", e))?;
            backend.migrate().await?;
            info!("using turso storage backend");
            Arc::new(backend)
        }
        other => anyhow::bail!("unknown storage backend: {}", other),
    };

    // 2. Initialize capability registry
    let registry = Arc::new(CapabilityRegistry::new(
        Duration::from_secs(10),
        Duration::from_secs(30),
    ));

    // 3. Initialize run worker
    let worker = Arc::new(RunWorker::new(
        storage.clone(),
        RunWorkerConfig::default(),
        QueueBackend::Memory,
    ));

    // 4. Build application state
    let state = Arc::new(AppState {
        storage,
        registry,
        worker,
        config: ServerConfig {
            enable_dev_routes: dev,
            ..Default::default()
        },
    });

    // 5. Build router
    let app = build_router(state.clone())
        .layer(CorsLayer::permissive())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(TraceLayer::new_for_http());

    // 6. Bind and serve
    let addr: SocketAddr = bind.parse()?;
    info!(%addr, dev, "reef server listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("reef server shut down gracefully");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received, starting graceful shutdown");
}
