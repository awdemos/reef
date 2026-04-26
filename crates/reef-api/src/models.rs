use chrono::{DateTime, Utc};
use reef_storage::models::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// OpenAI-compatible API types.
/// These are DTOs — thin wrappers around storage models with OpenAI-flavored serialization.

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateThreadRequest {
    pub messages: Option<Vec<CreateMessageRequest>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub role: String,
    pub content: String,
    pub attachments: Option<Vec<Attachment>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Attachment {
    pub file_id: String,
    pub tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadObject {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub metadata: serde_json::Value,
}

impl From<reef_storage::models::Thread> for ThreadObject {
    fn from(t: reef_storage::models::Thread) -> Self {
        Self {
            id: t.id.to_string(),
            object: "thread".into(),
            created_at: t.created_at.timestamp(),
            metadata: t.metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageObject {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub thread_id: String,
    pub role: String,
    pub content: Vec<MessageContent>,
    pub assistant_id: Option<String>,
    pub run_id: Option<String>,
    pub attachments: Vec<Attachment>,
    pub metadata: serde_json::Value,
}

impl From<reef_storage::models::Message> for MessageObject {
    fn from(m: reef_storage::models::Message) -> Self {
        Self {
            id: m.id.to_string(),
            object: "thread.message".into(),
            created_at: m.created_at.timestamp(),
            thread_id: m.thread_id.to_string(),
            role: m.role.clone(),
            content: m.content,
            assistant_id: None,
            run_id: m.run_id.map(|id| id.to_string()),
            attachments: vec![],
            metadata: m.metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateRunRequest {
    pub assistant_id: Option<String>,
    pub model: Option<String>,
    pub instructions: Option<String>,
    pub additional_instructions: Option<String>,
    pub tools: Option<Vec<ToolConfig>>,
    pub metadata: Option<serde_json::Value>,
    pub temperature: Option<f32>,
    pub max_prompt_tokens: Option<i32>,
    pub max_completion_tokens: Option<i32>,
    pub stream: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunObject {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub thread_id: String,
    pub assistant_id: Option<String>,
    pub status: String,
    pub model: String,
    pub instructions: Option<String>,
    pub tools: Vec<ToolConfig>,
    pub metadata: serde_json::Value,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub failed_at: Option<i64>,
    pub expired_at: Option<i64>,
    pub cancelled_at: Option<i64>,
    pub last_error: Option<RunError>,
}

impl From<reef_storage::models::Run> for RunObject {
    fn from(r: reef_storage::models::Run) -> Self {
        Self {
            id: r.id.to_string(),
            object: "thread.run".into(),
            created_at: r.created_at.timestamp(),
            thread_id: r.thread_id.to_string(),
            assistant_id: r.assistant_id.map(|id| id.to_string()),
            status: serde_json::to_string(&r.status).unwrap_or_default().trim_matches('"').to_string(),
            model: r.model,
            instructions: r.instructions,
            tools: r.tools,
            metadata: r.metadata,
            started_at: r.started_at.map(|t| t.timestamp()),
            completed_at: r.completed_at.map(|t| t.timestamp()),
            failed_at: r.failed_at.map(|t| t.timestamp()),
            expired_at: r.expired_at.map(|t| t.timestamp()),
            cancelled_at: None,
            last_error: r.last_error,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateAssistantRequest {
    pub model: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub tools: Option<Vec<ToolConfig>>,
    pub tool_resources: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub response_format: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantObject {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub model: String,
    pub instructions: Option<String>,
    pub tools: Vec<ToolConfig>,
    pub tool_resources: serde_json::Value,
    pub metadata: serde_json::Value,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub response_format: Option<serde_json::Value>,
}

impl From<reef_storage::models::Assistant> for AssistantObject {
    fn from(a: reef_storage::models::Assistant) -> Self {
        Self {
            id: a.id.to_string(),
            object: "assistant".into(),
            created_at: a.created_at.timestamp(),
            name: a.name,
            description: a.description,
            model: a.model,
            instructions: a.instructions,
            tools: a.tools,
            tool_resources: a.tool_resources,
            metadata: a.metadata,
            temperature: a.temperature,
            top_p: a.top_p,
            response_format: a.response_format,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub object: String,
    pub data: Vec<T>,
    pub first_id: Option<String>,
    pub last_id: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatCompletionMessage>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub n: Option<i32>,
    pub stream: Option<bool>,
    pub max_tokens: Option<i32>,
    pub presence_penalty: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub user: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionMessage {
    pub role: String,
    pub content: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionObject {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: i32,
    pub message: ChatCompletionMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorStoreObject {
    pub id: String,
    pub object: String,
    pub created_at: i64,
    pub name: Option<String>,
    pub bytes: i64,
    pub file_counts: FileCounts,
    pub status: String,
    pub expires_after: Option<serde_json::Value>,
    pub metadata: serde_json::Value,
}

impl From<reef_storage::models::VectorStore> for VectorStoreObject {
    fn from(v: reef_storage::models::VectorStore) -> Self {
        Self {
            id: v.id.to_string(),
            object: "vector_store".into(),
            created_at: v.created_at.timestamp(),
            name: v.name,
            bytes: v.bytes,
            file_counts: v.file_counts,
            status: serde_json::to_string(&v.status).unwrap_or_default().trim_matches('"').to_string(),
            expires_after: v.expires_after,
            metadata: v.metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileObjectApi {
    pub id: String,
    pub object: String,
    pub bytes: i64,
    pub created_at: i64,
    pub filename: String,
    pub purpose: String,
    pub status: String,
    pub status_details: Option<String>,
}

impl From<reef_storage::models::FileObject> for FileObjectApi {
    fn from(f: reef_storage::models::FileObject) -> Self {
        Self {
            id: f.id.to_string(),
            object: "file".into(),
            bytes: f.bytes,
            created_at: f.created_at.timestamp(),
            filename: f.filename,
            purpose: f.purpose,
            status: serde_json::to_string(&f.status).unwrap_or_default().trim_matches('"').to_string(),
            status_details: f.status_details,
        }
    }
}
