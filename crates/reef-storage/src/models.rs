use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Identity and authorization context for every storage operation.
/// Decoded from JWT or API key by the auth middleware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub user_id: String,
    pub tenant_id: Option<String>,
    pub scopes: Vec<Scope>,
}

impl Principal {
    pub fn is_system() -> Self {
        Self {
            user_id: "__system__".to_string(),
            tenant_id: None,
            scopes: vec![Scope::System],
        }
    }

    pub fn has_scope(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope) || self.scopes.contains(&Scope::System)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    System,
    Read,
    Write,
    Admin,
    FileSearch,
    ApiKeyManage,
}

/// Marker trait for entities that can be stored.
/// In a real implementation this would carry metadata like table name,
/// but we keep it minimal for clarity.
pub trait Entity: Send + Sync + 'static {
    fn id(&self) -> Uuid;
    fn owner_id(&self) -> &str;
    fn created_at(&self) -> DateTime<Utc>;
    fn updated_at(&self) -> DateTime<Utc>;
}

macro_rules! define_entity {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct $name {
            pub id: Uuid,
            pub owner_id: String,
            pub created_at: DateTime<Utc>,
            pub updated_at: DateTime<Utc>,
            $(pub $field: $ty),*
        }

        impl Entity for $name {
            fn id(&self) -> Uuid { self.id }
            fn owner_id(&self) -> &str { &self.owner_id }
            fn created_at(&self) -> DateTime<Utc> { self.created_at }
            fn updated_at(&self) -> DateTime<Utc> { self.updated_at }
        }
    };
}

define_entity!(Thread {
    title: Option<String>,
    metadata: serde_json::Value,
});

define_entity!(Message {
    thread_id: Uuid,
    run_id: Option<Uuid>,
    role: String,
    content: Vec<MessageContent>,
    annotations: Vec<Annotation>,
    metadata: serde_json::Value,
});

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageContent {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    Refusal { refusal: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Annotation {
    pub file_id: String,
    pub quote: String,
    pub start_index: usize,
    pub end_index: usize,
}

/// A Run is the event-sourced orchestration unit.
/// State transitions are driven by RunEvents, not direct mutation.
define_entity!(Run {
    thread_id: Uuid,
    assistant_id: Option<Uuid>,
    status: RunStatus,
    model: String,
    instructions: Option<String>,
    tools: Vec<ToolConfig>,
    temperature: Option<f32>,
    max_prompt_tokens: Option<i32>,
    max_completion_tokens: Option<i32>,
    metadata: serde_json::Value,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    expired_at: Option<DateTime<Utc>>,
    failed_at: Option<DateTime<Utc>>,
    last_error: Option<RunError>,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    InProgress,
    RequiresAction,
    Cancelling,
    Cancelled,
    Failed,
    Completed,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolConfig {
    pub r#type: String,
    pub config: serde_json::Value,
}

/// Immutable event in a Run's event log.
/// Append-only. The current state is a fold over events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunEvent {
    pub id: Uuid,
    pub run_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub event_type: RunEventType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunEventType {
    RunCreated,
    RetrievalStarted { query: String },
    RetrievalCompleted { chunks: Vec<Uuid> },
    ContextAssembled { token_count: usize },
    GenerationStarted,
    TokenStreamed { delta: String, index: usize },
    ToolCallCreated { tool_call_id: String, tool_type: String, arguments: String },
    ToolCallCompleted { tool_call_id: String, output: String },
    RunCompleted,
    RunFailed { error: RunError },
    RunCancelled,
}

define_entity!(Assistant {
    name: Option<String>,
    description: Option<String>,
    model: String,
    instructions: Option<String>,
    tools: Vec<ToolConfig>,
    tool_resources: serde_json::Value,
    metadata: serde_json::Value,
    temperature: Option<f32>,
    top_p: Option<f32>,
    response_format: Option<serde_json::Value>,
});

define_entity!(VectorStore {
    name: Option<String>,
    bytes: i64,
    file_counts: FileCounts,
    status: VectorStoreStatus,
    expires_after: Option<serde_json::Value>,
    metadata: serde_json::Value,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FileCounts {
    pub in_progress: i32,
    pub completed: i32,
    pub failed: i32,
    pub cancelled: i32,
    pub total: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VectorStoreStatus {
    InProgress,
    Completed,
    Failed,
}

/// A single indexed chunk inside a vector store.
define_entity!(VectorChunk {
    vector_store_id: Uuid,
    file_id: Uuid,
    embedding_model: String,
    dimension: usize,
    embedding: Vec<f32>,
    content: String,
    content_type: ChunkContentType,
    metadata: serde_json::Value,
    index: usize,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkContentType {
    Text,
    Table,
    ImageCaption,
    AudioTranscript,
}

define_entity!(FileObject {
    bytes: i64,
    filename: String,
    purpose: String,
    status: FileStatus,
    status_details: Option<String>,
    metadata: serde_json::Value,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Uploaded,
    Processed,
    Error,
}

define_entity!(ApiKey {
    key_hash: String,
    key_preview: String,
    name: String,
    scopes: Vec<Scope>,
    expires_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
});

/// State machine for file ingestion into a vector store.
define_entity!(IngestionJob {
    vector_store_id: Uuid,
    file_id: Uuid,
    status: IngestionStatus,
    stage: IngestionStage,
    error: Option<String>,
    progress_percent: i32,
});

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionStage {
    Download,
    Parse,
    Chunk,
    Embed,
    Index,
}
