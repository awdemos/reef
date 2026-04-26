pub mod grpc;
pub mod embedded_llama;

use std::pin::Pin;

use async_trait::async_trait;
use reef_proto::{
    chat::{ChatCompletionRequest, ChatCompletionResponse},
    embeddings::{EmbeddingRequest, EmbeddingResponse},
};
use tokio_stream::Stream;

use crate::registry::{HealthStatus, ModelCapability};

/// A token delta in a streaming response.
#[derive(Debug, Clone, PartialEq)]
pub struct TokenDelta {
    pub index: usize,
    pub delta: String,
    pub finish_reason: Option<String>,
}

/// Unified interface for any inference backend — embedded or external.
#[async_trait]
pub trait InferenceBackend: Send + Sync + 'static {
    /// Unique identifier for this backend instance.
    fn instance_id(&self) -> &str;

    /// Current capability advertisement.
    fn capability(&self) -> &ModelCapability;

    /// Health status as of last check.
    fn health(&self) -> HealthStatus;

    /// Non-streaming chat completion.
    async fn chat_complete(&self, req: ChatCompletionRequest) -> anyhow::Result<ChatCompletionResponse>;

    /// Streaming chat completion.
    async fn chat_complete_stream(
        &self,
        req: ChatCompletionRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<TokenDelta>> + Send>>>;

    /// Generate embeddings.
    async fn embed(&self, req: EmbeddingRequest) -> anyhow::Result<EmbeddingResponse>;

    /// Count tokens (optional — falls back to approximation if unavailable).
    async fn count_tokens(&self, text: &str) -> anyhow::Result<usize> {
        let _ = text;
        Ok(0)
    }
}

/// Owned backend instance that can be registered in the capability registry.
pub enum BackendInstance {
    ExternalGrpc(grpc::GrpcBackend),
    EmbeddedLlama(embedded_llama::LlamaBackend),
}

impl BackendInstance {
    pub fn as_inference_backend(&self) -> &dyn InferenceBackend {
        match self {
            BackendInstance::ExternalGrpc(b) => b,
            BackendInstance::EmbeddedLlama(b) => b,
        }
    }
}
