use std::sync::Arc;
use std::convert::Infallible;

use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Sse},
    Json,
};
use chrono::Utc;
use futures::StreamExt;
use reef_engine::backends::InferenceBackend;
use reef_proto::{
    chat::{ChatCompletionRequest as ProtoChatRequest, ChatItem, ChatRole},
    embeddings::{EmbeddingRequest as ProtoEmbedRequest},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, error, warn};

use crate::{
    error::{ApiError, ApiResult},
    middleware::AuthPrincipal,
    models::*,
    state::AppState,
};

pub async fn chat_completions(
    State(state): State<Arc<AppState>>,
    principal: AuthPrincipal,
    Json(req): Json<ChatCompletionRequest>,
) -> ApiResult<axum::response::Response> {
    // Convert OpenAI request to proto request
    let chat_items: Vec<ChatItem> = req
        .messages
        .into_iter()
        .map(|m| ChatItem {
            role: match m.role.as_str() {
                "system" => ChatRole::System as i32,
                "assistant" => ChatRole::Assistant as i32,
                _ => ChatRole::User as i32,
            },
            content: m.content.unwrap_or_default(),
        })
        .collect();

    let proto_req = ProtoChatRequest {
        chat_items,
        max_new_tokens: req.max_tokens.unwrap_or(1024),
        temperature: req.temperature,
        top_k: None,
        top_p: req.top_p,
        do_sample: Some(true),
        n: req.n,
        stop: vec![],
        repetition_penalty: None,
        presence_penalty: req.presence_penalty,
        frequency_penalty: req.frequency_penalty,
        best_of: None,
        logit_bias: Default::default(),
        return_full_text: None,
        truncate: None,
        typical_p: None,
        watermark: None,
        seed: None,
        user: req.user,
    };

    // Resolve backend
    let backend = state.registry.resolve(&req.model)
        .ok_or_else(|| ApiError::BadRequest(format!("model '{}' not available", req.model)))?;

    if req.stream == Some(true) {
        let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(32);
        let model = req.model.clone();

        tokio::spawn(async move {
            let stream_result = backend.chat_complete_stream(proto_req).await;
            match stream_result {
                Ok(mut stream) => {
                    let mut index = 0;
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(delta) => {
                                let chunk = serde_json::json!({
                                    "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                                    "object": "chat.completion.chunk",
                                    "created": Utc::now().timestamp(),
                                    "model": model,
                                    "choices": [{
                                        "index": delta.index,
                                        "delta": {"content": delta.delta},
                                        "finish_reason": delta.finish_reason,
                                    }],
                                });
                                if tx.send(Ok(Event::default().data(chunk.to_string()))).await.is_err() {
                                    break;
                                }
                                index = delta.index;
                            }
                            Err(e) => {
                                warn!(error = %e, "stream error");
                                break;
                            }
                        }
                    }
                    let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                }
                Err(e) => {
                    error!(error = %e, "failed to start stream");
                    let err = serde_json::json!({"error": e.to_string()});
                    let _ = tx.send(Ok(Event::default().data(err.to_string()))).await;
                }
            }
        });

        let stream = ReceiverStream::new(rx);
        Ok(Sse::new(stream).into_response())
    } else {
        // Non-streaming: collect the stream into a single response
        let mut content = String::new();
        let stream_result = backend.chat_complete_stream(proto_req).await
            .map_err(|e| ApiError::Internal)?;
        
        tokio::pin!(stream_result);
        while let Some(result) = stream_result.next().await {
            match result {
                Ok(delta) => content.push_str(&delta.delta),
                Err(_) => break,
            }
        }

        let resp = ChatCompletionObject {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".into(),
            created: Utc::now().timestamp(),
            model: req.model.clone(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: "assistant".into(),
                    content: Some(content),
                    name: None,
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            }),
        };

        Ok(Json(resp).into_response())
    }
}

pub async fn create_embeddings(
    State(state): State<Arc<AppState>>,
    _principal: AuthPrincipal,
    Json(req): Json<EmbeddingRequest>,
) -> ApiResult<Json<EmbeddingResponse>> {
    let backend = state.registry.resolve(&req.model)
        .ok_or_else(|| ApiError::BadRequest(format!("model '{}' not available", req.model)))?;

    let proto_req = ProtoEmbedRequest {
        inputs: req.input,
    };

    let result = backend.embed(proto_req).await
        .map_err(|e| ApiError::Internal)?;

    let data: Vec<EmbeddingData> = result.embeddings.into_iter().enumerate().map(|(i, e)| {
        EmbeddingData {
            object: "embedding".into(),
            embedding: e.embedding,
            index: i as i32,
        }
    }).collect();

    Ok(Json(EmbeddingResponse {
        model: req.model,
        data,
    }))
}

// Local types for OpenAI-compatible embedding request/response
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EmbeddingResponse {
    pub model: String,
    pub data: Vec<EmbeddingData>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EmbeddingData {
    pub object: String,
    pub embedding: Vec<f32>,
    pub index: i32,
}
