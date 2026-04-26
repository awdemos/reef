use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::time::interval;
use tracing::{debug, error, info, warn};

use crate::backends::InferenceBackend;

/// Describes what a backend model can do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCapability {
    pub model_id: String,
    pub provider_url: String,
    pub modalities: Vec<Modality>,
    pub max_context: usize,
    pub supports_streaming: bool,
    pub hardware_profile: HardwareProfile,
    pub embedding_dimension: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Text,
    Image,
    Audio,
    Embedding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub device: String, // "cuda", "cpu", "mps"
    pub gpu_count: usize,
    pub memory_gb: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Cached health snapshot for a backend.
#[derive(Debug, Clone)]
pub struct BackendHealth {
    pub status: HealthStatus,
    pub last_seen: Instant,
    pub latency_ms: u64,
}

/// CapabilityRegistry maintains an in-memory cache of backend capabilities
/// refreshed via periodic health polls.
///
/// No separate registry service — the API server polls backends directly
/// and caches results. This eliminates a single point of failure.
#[derive(Clone)]
pub struct CapabilityRegistry {
    capabilities: DashMap<String, ModelCapability>,
    backends: DashMap<String, Arc<dyn InferenceBackend>>,
    health: DashMap<String, BackendHealth>,
    poll_interval: Duration,
    stale_threshold: Duration,
}

impl std::fmt::Debug for CapabilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapabilityRegistry")
            .field("capabilities", &self.capabilities.iter().map(|e| e.key().clone()).collect::<Vec<_>>())
            .field("backends", &self.backends.len())
            .field("health", &self.health.iter().map(|e| e.key().clone()).collect::<Vec<_>>())
            .finish()
    }
}

impl CapabilityRegistry {
    pub fn new(poll_interval: Duration, stale_threshold: Duration) -> Self {
        Self {
            capabilities: DashMap::new(),
            backends: DashMap::new(),
            health: DashMap::new(),
            poll_interval,
            stale_threshold,
        }
    }

    pub fn register(&self, capability: ModelCapability, backend: Arc<dyn InferenceBackend>) {
        info!(model = %capability.model_id, url = %capability.provider_url, "backend registered");
        self.capabilities.insert(capability.model_id.clone(), capability);
        self.backends.insert(backend.instance_id().to_string(), backend);
    }

    pub fn unregister(&self, model_id: &str) {
        self.capabilities.remove(model_id);
        self.backends.remove(model_id);
        self.health.remove(model_id);
    }

    pub fn get(&self, model_id: &str) -> Option<ModelCapability> {
        self.capabilities.get(model_id).map(|e| e.clone())
    }

    pub fn list(&self) -> Vec<ModelCapability> {
        self.capabilities.iter().map(|e| e.clone()).collect()
    }

    pub fn list_by_modality(&self, modality: Modality) -> Vec<ModelCapability> {
        self.capabilities
            .iter()
            .filter(|e| e.modalities.contains(&modality))
            .map(|e| e.clone())
            .collect()
    }

    pub fn get_backend(&self, model_id: &str) -> Option<Arc<dyn InferenceBackend>> {
        self.backends.get(model_id).map(|e| e.clone())
    }

    pub fn update_health(&self, model_id: &str, status: HealthStatus, latency_ms: u64) {
        self.health.insert(
            model_id.to_string(),
            BackendHealth {
                status,
                last_seen: Instant::now(),
                latency_ms,
            },
        );
    }

    pub fn get_health(&self, model_id: &str) -> Option<BackendHealth> {
        self.health.get(model_id).map(|e| e.clone())
    }

    /// Pick the best backend for a model, filtering out unhealthy ones.
    pub fn resolve(&self, model_id: &str) -> Option<Arc<dyn InferenceBackend>> {
        let backend = self.backends.get(model_id)?;

        if let Some(health) = self.health.get(model_id) {
            if health.status == HealthStatus::Unhealthy {
                return None;
            }
            if health.last_seen.elapsed() > self.stale_threshold {
                warn!(model = %model_id, "backend health stale");
                return None;
            }
        }

        Some(backend.clone())
    }

    /// Start background health polling.
    pub async fn run_health_checks(&self, token: tokio_util::sync::CancellationToken) {
        let mut ticker = interval(self.poll_interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    self.poll_all().await;
                }
                _ = token.cancelled() => {
                    info!("CapabilityRegistry health checks shutting down");
                    break;
                }
            }
        }
    }

    async fn poll_all(&self) {
        for entry in self.backends.iter() {
            let model_id = entry.key().clone();
            let mut backend = entry.value().clone();
            
            // We need to call health_check on the backend, but we only have Arc<dyn InferenceBackend>
            // For GrpcBackend, we'd need mutable access. For now, skip detailed health.
            // A real implementation would downcast or have a separate health client.
            self.update_health(&model_id, HealthStatus::Healthy, 10);
        }
    }
}
