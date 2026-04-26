use std::pin::Pin;

use async_trait::async_trait;
use reef_proto::{
    chat::{ChatCompletionRequest, ChatCompletionResponse, chat_completion_stream_service_client::ChatCompletionStreamServiceClient},
    embeddings::{EmbeddingRequest, EmbeddingResponse, embeddings_service_client::EmbeddingsServiceClient},
    name::{NameResponse, name_service_client::NameServiceClient},
};
use tokio_stream::Stream;
use tonic::transport::Channel;
use tracing::{debug, warn};

use crate::backends::{InferenceBackend, TokenDelta};
use crate::registry::{HealthStatus, ModelCapability};

/// External backend accessed via gRPC (vLLM, llama-cpp-python, whisper, etc.).
#[derive(Debug, Clone)]
pub struct GrpcBackend {
    instance_id: String,
    capability: ModelCapability,
    health: HealthStatus,
    endpoint: String,
    chat_stream_client: Option<ChatCompletionStreamServiceClient<Channel>>,
    embed_client: Option<EmbeddingsServiceClient<Channel>>,
    name_client: Option<NameServiceClient<Channel>>,
}

impl GrpcBackend {
    pub async fn new(instance_id: String, capability: ModelCapability, endpoint: String) -> anyhow::Result<Self> {
        let channel = Channel::from_shared(endpoint.clone())
            .map_err(|e| anyhow::anyhow!("invalid endpoint: {}", e))?
            .connect()
            .await
            .map_err(|e| anyhow::anyhow!("connection failed: {}", e))?;

        let chat_stream_client = ChatCompletionStreamServiceClient::new(channel.clone());
        let embed_client = EmbeddingsServiceClient::new(channel.clone());
        let name_client = NameServiceClient::new(channel);

        Ok(Self {
            instance_id,
            capability,
            health: HealthStatus::Healthy,
            endpoint,
            chat_stream_client: Some(chat_stream_client),
            embed_client: Some(embed_client),
            name_client: Some(name_client),
        })
    }

    pub fn set_health(&mut self, health: HealthStatus) {
        self.health = health;
    }

    /// Perform a health check by calling the NameService.
    pub async fn health_check(&mut self) -> HealthStatus {
        if let Some(ref mut client) = self.name_client {
            match client.name(()).await {
                Ok(response) => {
                    let _name = response.into_inner().name;
                    debug!(instance = %self.instance_id, "health check ok");
                    HealthStatus::Healthy
                }
                Err(e) => {
                    warn!(instance = %self.instance_id, error = %e, "health check failed");
                    HealthStatus::Unhealthy
                }
            }
        } else {
            HealthStatus::Unknown
        }
    }
}

#[async_trait]
impl InferenceBackend for GrpcBackend {
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
        anyhow::bail!("non-streaming chat not implemented for gRPC backend — use chat_complete_stream")
    }

    async fn chat_complete_stream(
        &self,
        req: ChatCompletionRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = anyhow::Result<TokenDelta>> + Send>>> {
        debug!(model = %self.instance_id, endpoint = %self.endpoint, "gRPC chat_complete_stream");
        
        let mut client = self.chat_stream_client.clone()
            .ok_or_else(|| anyhow::anyhow!("chat stream client not initialized"))?;

        let request = tonic::Request::new(req);
        let response = client.chat_complete_stream(request).await?;
        let mut stream = response.into_inner();

        let stream = async_stream::try_stream! {
            let mut index = 0;
            while let Some(item) = stream.message().await? {
                for choice in item.choices {
                    let delta = choice.chat_item.map(|ci| ci.content).unwrap_or_default();
                    let finish_reason = match choice.finish_reason {
                        1 => Some("stop".to_string()),
                        2 => Some("length".to_string()),
                        _ => None,
                    };
                    yield TokenDelta { index, delta, finish_reason };
                    index += 1;
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn embed(&self, req: EmbeddingRequest) -> anyhow::Result<EmbeddingResponse> {
        debug!(model = %self.instance_id, inputs = req.inputs.len(), "gRPC embed");
        
        let mut client = self.embed_client.clone()
            .ok_or_else(|| anyhow::anyhow!("embed client not initialized"))?;

        let request = tonic::Request::new(req);
        let response = client.create_embedding(request).await?;
        Ok(response.into_inner())
    }
}
