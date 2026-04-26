use std::sync::Arc;

use reef_engine::{CapabilityRegistry, RunWorker, RunWorkerConfig};
use reef_storage::StorageBackend;

/// Shared application state for Axum handlers.
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<dyn StorageBackend>,
    pub registry: Arc<CapabilityRegistry>,
    pub worker: Arc<RunWorker>,
    pub config: ServerConfig,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("registry", &self.registry)
            .field("config", &self.config)
            .field("storage", &"<dyn StorageBackend>")
            .field("worker", &"<RunWorker>")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub enable_dev_routes: bool,
    pub max_request_size: usize,
    pub request_timeout_secs: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            enable_dev_routes: false,
            max_request_size: 50 * 1024 * 1024, // 50MB
            request_timeout_secs: 120,
        }
    }
}
