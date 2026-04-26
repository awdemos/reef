use std::pin::Pin;

use async_trait::async_trait;
use reef_proto::{
    chat::{ChatCompletionRequest, ChatCompletionResponse},
    embeddings::{EmbeddingRequest, EmbeddingResponse},
};
use tokio_stream::Stream;
use tracing::{debug, error, info, warn};

use crate::backends::{InferenceBackend, TokenDelta};
use crate::registry::{HealthStatus, HardwareProfile, ModelCapability, Modality};

/// Embedded backend using llama.cpp (via llama-cpp-rs when enabled).
#[derive(Debug, Clone)]
pub struct LlamaBackend {
    instance_id: String,
    capability: ModelCapability,
    model_path: std::path::PathBuf,
    health: HealthStatus,
}

impl LlamaBackend {
    pub fn new(instance_id: String, model_path: std::path::PathBuf) -> Self {
        let capability = ModelCapability {
            model_id: instance_id.clone(),
            provider_url: format!("embedded://{}", instance_id),
            modalities: vec![Modality::Text],
            max_context: 8192,
            supports_streaming: true,
            hardware_profile: HardwareProfile {
                device: "cpu".into(),
                gpu_count: 0,
                memory_gb: 0,
            },
            embedding_dimension: None,
        };

        Self {
            instance_id,
            capability,
            model_path,
            health: HealthStatus::Unknown,
        }
    }

    pub async fn load(&mut self) -> anyhow::Result<()> {
        info!(
            instance = %self.instance_id,
            path = %self.model_path.display(),
            "loading embedded llama model"
        );

        if !self.model_path.exists() {
            anyhow::bail!("model file not found: {}", self.model_path.display());
        }

        #[cfg(not(feature = "llama"))]
        {
            info!("llama feature not enabled; backend loaded in stub mode");
        }

        self.health = HealthStatus::Healthy;
        Ok(())
    }

    pub fn set_health(&mut self, health: HealthStatus) {
        self.health = health;
    }
}

#[async_trait]
impl InferenceBackend for LlamaBackend {
    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capability(&self) -> &ModelCapability {
        &self.capability
    }

    fn health(&self) -> HealthStatus {
        self.health.clone()
    }

    async fn chat_complete(&self, _req: ChatCompletionRequest) -> anyhow::Result<ChatCompletionResponse> {
        debug!(model = %self.instance_id, "embedded llama chat_complete");
        anyhow::bail!(
            "embedded llama backend is a stub. Rebuild reef with --features llama to enable native inference."
        )
    }

    async fn chat_complete_stream(
        &self,
        _req: ChatCompletionRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<TokenDelta>> + Send>>> {
        debug!(model = %self.instance_id, "embedded llama chat_complete_stream");
        anyhow::bail!(
            "embedded llama backend is a stub. Rebuild reef with --features llama to enable native inference."
        )
    }

    async fn embed(&self, _req: EmbeddingRequest) -> anyhow::Result<EmbeddingResponse> {
        debug!(model = %self.instance_id, "embedded llama embed");
        anyhow::bail!(
            "embedded llama backend is a stub. Rebuild reef with --features llama to enable native inference."
        )
    }
}
