use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use parking_lot::RwLock;
use uuid::Uuid;

use crate::{
    backend::StorageBackend,
    error::{StorageError, StorageResult},
    models::*,
    query::{Page, QuerySpec, ScoredChunk, VectorQuery},
};

/// In-memory storage backend for tests and local dev.
/// Not intended for production use.
#[derive(Debug, Default, Clone)]
pub struct MemoryBackend {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug, Default)]
struct Inner {
    threads: HashMap<Uuid, Thread>,
    messages: HashMap<Uuid, Message>,
    runs: HashMap<Uuid, Run>,
    run_events: HashMap<Uuid, Vec<RunEvent>>,
    assistants: HashMap<Uuid, Assistant>,
    vector_stores: HashMap<Uuid, VectorStore>,
    vector_chunks: HashMap<Uuid, VectorChunk>,
    files: HashMap<Uuid, FileObject>,
    api_keys: HashMap<Uuid, ApiKey>,
    ingestion_jobs: HashMap<Uuid, IngestionJob>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn authorize<E: crate::Entity>(principal: &Principal, entity: &E) -> StorageResult<()> {
        if principal.user_id != entity.owner_id() && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }
        Ok(())
    }
}

#[async_trait]
impl StorageBackend for MemoryBackend {
    async fn ping(&self) -> StorageResult<()> {
        Ok(())
    }

    // ---- Threads ----
    async fn create_thread(&self, _principal: &Principal, thread: &Thread) -> StorageResult<Thread> {
        let mut inner = self.inner.write();
        if inner.threads.contains_key(&thread.id) {
            return Err(StorageError::Conflict("thread exists".into()));
        }
        inner.threads.insert(thread.id, thread.clone());
        Ok(thread.clone())
    }

    async fn get_thread(&self, principal: &Principal, id: Uuid) -> StorageResult<Thread> {
        let inner = self.inner.read();
        let thread = inner.threads.get(&id).ok_or_else(|| StorageError::not_found("thread"))?;
        Self::authorize(principal, thread)?;
        Ok(thread.clone())
    }

    async fn update_thread(&self, principal: &Principal, thread: &Thread) -> StorageResult<Thread> {
        let mut inner = self.inner.write();
        if !inner.threads.contains_key(&thread.id) {
            return Err(StorageError::not_found("thread"));
        }
        Self::authorize(principal, thread)?;
        let mut updated = thread.clone();
        updated.updated_at = Utc::now();
        inner.threads.insert(thread.id, updated.clone());
        Ok(updated)
    }

    async fn delete_thread(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        let mut inner = self.inner.write();
        let thread = inner.threads.get(&id).ok_or_else(|| StorageError::not_found("thread"))?;
        Self::authorize(principal, thread)?;
        inner.threads.remove(&id);
        Ok(())
    }

    async fn list_threads(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<Thread>> {
        let inner = self.inner.read();
        let mut items: Vec<Thread> = inner
            .threads
            .values()
            .filter(|t| t.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        // Simple ordering
        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    // ---- Messages ----
    async fn create_message(&self, _principal: &Principal, message: &Message) -> StorageResult<Message> {
        let mut inner = self.inner.write();
        inner.messages.insert(message.id, message.clone());
        Ok(message.clone())
    }

    async fn get_message(&self, principal: &Principal, id: Uuid) -> StorageResult<Message> {
        let inner = self.inner.read();
        let msg = inner.messages.get(&id).ok_or_else(|| StorageError::not_found("message"))?;
        Self::authorize(principal, msg)?;
        Ok(msg.clone())
    }

    async fn list_messages(&self, principal: &Principal, thread_id: Uuid, spec: &QuerySpec) -> StorageResult<Page<Message>> {
        let inner = self.inner.read();
        let mut items: Vec<Message> = inner
            .messages
            .values()
            .filter(|m| m.thread_id == thread_id)
            .filter(|m| m.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    // ---- Runs ----
    async fn create_run(&self, _principal: &Principal, run: &Run) -> StorageResult<Run> {
        let mut inner = self.inner.write();
        inner.runs.insert(run.id, run.clone());
        Ok(run.clone())
    }

    async fn get_run(&self, principal: &Principal, id: Uuid) -> StorageResult<Run> {
        let inner = self.inner.read();
        let run = inner.runs.get(&id).ok_or_else(|| StorageError::not_found("run"))?;
        Self::authorize(principal, run)?;
        Ok(run.clone())
    }

    async fn update_run(&self, principal: &Principal, run: &Run) -> StorageResult<Run> {
        let mut inner = self.inner.write();
        if !inner.runs.contains_key(&run.id) {
            return Err(StorageError::not_found("run"));
        }
        Self::authorize(principal, run)?;
        let mut updated = run.clone();
        updated.updated_at = Utc::now();
        inner.runs.insert(run.id, updated.clone());
        Ok(updated)
    }

    async fn list_runs(&self, principal: &Principal, thread_id: Uuid, spec: &QuerySpec) -> StorageResult<Page<Run>> {
        let inner = self.inner.read();
        let mut items: Vec<Run> = inner
            .runs
            .values()
            .filter(|r| r.thread_id == thread_id)
            .filter(|r| r.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    // ---- Assistants ----
    async fn create_assistant(&self, _principal: &Principal, assistant: &Assistant) -> StorageResult<Assistant> {
        let mut inner = self.inner.write();
        inner.assistants.insert(assistant.id, assistant.clone());
        Ok(assistant.clone())
    }

    async fn get_assistant(&self, principal: &Principal, id: Uuid) -> StorageResult<Assistant> {
        let inner = self.inner.read();
        let a = inner.assistants.get(&id).ok_or_else(|| StorageError::not_found("assistant"))?;
        Self::authorize(principal, a)?;
        Ok(a.clone())
    }

    async fn update_assistant(&self, principal: &Principal, assistant: &Assistant) -> StorageResult<Assistant> {
        let mut inner = self.inner.write();
        if !inner.assistants.contains_key(&assistant.id) {
            return Err(StorageError::not_found("assistant"));
        }
        Self::authorize(principal, assistant)?;
        let mut updated = assistant.clone();
        updated.updated_at = Utc::now();
        inner.assistants.insert(assistant.id, updated.clone());
        Ok(updated)
    }

    async fn delete_assistant(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        let mut inner = self.inner.write();
        let a = inner.assistants.get(&id).ok_or_else(|| StorageError::not_found("assistant"))?;
        Self::authorize(principal, a)?;
        inner.assistants.remove(&id);
        Ok(())
    }

    async fn list_assistants(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<Assistant>> {
        let inner = self.inner.read();
        let mut items: Vec<Assistant> = inner
            .assistants
            .values()
            .filter(|a| a.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    // ---- Vector Stores ----
    async fn create_vector_store(&self, _principal: &Principal, store: &VectorStore) -> StorageResult<VectorStore> {
        let mut inner = self.inner.write();
        inner.vector_stores.insert(store.id, store.clone());
        Ok(store.clone())
    }

    async fn get_vector_store(&self, principal: &Principal, id: Uuid) -> StorageResult<VectorStore> {
        let inner = self.inner.read();
        let s = inner.vector_stores.get(&id).ok_or_else(|| StorageError::not_found("vector_store"))?;
        Self::authorize(principal, s)?;
        Ok(s.clone())
    }

    async fn delete_vector_store(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        let mut inner = self.inner.write();
        let s = inner.vector_stores.get(&id).ok_or_else(|| StorageError::not_found("vector_store"))?;
        Self::authorize(principal, s)?;
        inner.vector_stores.remove(&id);
        inner.vector_chunks.retain(|_, c| c.vector_store_id != id);
        Ok(())
    }

    async fn list_vector_stores(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<VectorStore>> {
        let inner = self.inner.read();
        let mut items: Vec<VectorStore> = inner
            .vector_stores
            .values()
            .filter(|s| s.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    // ---- Files ----
    async fn create_file(&self, _principal: &Principal, file: &FileObject) -> StorageResult<FileObject> {
        let mut inner = self.inner.write();
        inner.files.insert(file.id, file.clone());
        Ok(file.clone())
    }

    async fn get_file(&self, principal: &Principal, id: Uuid) -> StorageResult<FileObject> {
        let inner = self.inner.read();
        let f = inner.files.get(&id).ok_or_else(|| StorageError::not_found("file"))?;
        Self::authorize(principal, f)?;
        Ok(f.clone())
    }

    async fn delete_file(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        let mut inner = self.inner.write();
        let f = inner.files.get(&id).ok_or_else(|| StorageError::not_found("file"))?;
        Self::authorize(principal, f)?;
        inner.files.remove(&id);
        Ok(())
    }

    async fn list_files(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<FileObject>> {
        let inner = self.inner.read();
        let mut items: Vec<FileObject> = inner
            .files
            .values()
            .filter(|f| f.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    // ---- API Keys ----
    async fn create_api_key(&self, _principal: &Principal, key: &ApiKey) -> StorageResult<ApiKey> {
        let mut inner = self.inner.write();
        inner.api_keys.insert(key.id, key.clone());
        Ok(key.clone())
    }

    async fn get_api_key_by_hash(&self, _principal: &Principal, hash: &str) -> StorageResult<ApiKey> {
        let inner = self.inner.read();
        inner
            .api_keys
            .values()
            .find(|k| k.key_hash == hash)
            .cloned()
            .ok_or_else(|| StorageError::not_found("api_key"))
    }

    async fn list_api_keys(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<ApiKey>> {
        let inner = self.inner.read();
        let mut items: Vec<ApiKey> = inner
            .api_keys
            .values()
            .filter(|k| k.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }

    async fn delete_api_key(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        let mut inner = self.inner.write();
        let k = inner.api_keys.get(&id).ok_or_else(|| StorageError::not_found("api_key"))?;
        Self::authorize(principal, k)?;
        inner.api_keys.remove(&id);
        Ok(())
    }

    // ---- Tier 2: Vector Search ----
    async fn vector_search(&self, principal: &Principal, query: &VectorQuery) -> StorageResult<Vec<ScoredChunk>> {
        let inner = self.inner.read();

        // Verify vector store access
        let store = inner
            .vector_stores
            .get(&query.vector_store_id)
            .ok_or_else(|| StorageError::not_found("vector_store"))?;
        Self::authorize(principal, store)?;

        let mut scored: Vec<(f32, VectorChunk)> = inner
            .vector_chunks
            .values()
            .filter(|c| c.vector_store_id == query.vector_store_id)
            .map(|c| {
                let score = cosine_similarity(&query.embedding, &c.embedding);
                (score, c.clone())
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

        let results: Vec<ScoredChunk> = scored
            .into_iter()
            .take(query.top_k)
            .filter(|(score, _)| query.min_score.map_or(true, |min| *score >= min))
            .map(|(score, c)| ScoredChunk {
                chunk_id: c.id,
                vector_store_id: c.vector_store_id,
                file_id: c.file_id,
                score,
                content: c.content.clone(),
                content_type: c.content_type.clone(),
                metadata: c.metadata.clone(),
                index: c.index,
            })
            .collect();

        Ok(results)
    }

    async fn create_vector_chunks(
        &self,
        _principal: &Principal,
        chunks: &[VectorChunk],
    ) -> StorageResult<Vec<VectorChunk>> {
        let mut inner = self.inner.write();
        for chunk in chunks {
            inner.vector_chunks.insert(chunk.id, chunk.clone());
        }
        Ok(chunks.to_vec())
    }

    async fn delete_vector_chunks_by_file(
        &self,
        _principal: &Principal,
        _vector_store_id: Uuid,
        file_id: Uuid,
    ) -> StorageResult<u64> {
        let mut inner = self.inner.write();
        let before = inner.vector_chunks.len();
        inner.vector_chunks.retain(|_, c| c.file_id != file_id);
        let removed = (before - inner.vector_chunks.len()) as u64;
        Ok(removed)
    }

    // ---- Tier 2: Event Sourcing ----
    async fn append_run_event(&self, _principal: &Principal, event: &RunEvent) -> StorageResult<RunEvent> {
        let mut inner = self.inner.write();
        inner
            .run_events
            .entry(event.run_id)
            .or_default()
            .push(event.clone());
        Ok(event.clone())
    }

    async fn get_run_events(&self, principal: &Principal, run_id: Uuid) -> StorageResult<Vec<RunEvent>> {
        let inner = self.inner.read();
        let run = inner.runs.get(&run_id).ok_or_else(|| StorageError::not_found("run"))?;
        Self::authorize(principal, run)?;
        let events = inner.run_events.get(&run_id).cloned().unwrap_or_default();
        Ok(events)
    }

    // ---- Tier 2: Ingestion Jobs ----
    async fn create_ingestion_job(&self, _principal: &Principal, job: &IngestionJob) -> StorageResult<IngestionJob> {
        let mut inner = self.inner.write();
        inner.ingestion_jobs.insert(job.id, job.clone());
        Ok(job.clone())
    }

    async fn update_ingestion_job(&self, principal: &Principal, job: &IngestionJob) -> StorageResult<IngestionJob> {
        let mut inner = self.inner.write();
        if !inner.ingestion_jobs.contains_key(&job.id) {
            return Err(StorageError::not_found("ingestion_job"));
        }
        Self::authorize(principal, job)?;
        let mut updated = job.clone();
        updated.updated_at = Utc::now();
        inner.ingestion_jobs.insert(job.id, updated.clone());
        Ok(updated)
    }

    async fn get_ingestion_job(&self, principal: &Principal, id: Uuid) -> StorageResult<IngestionJob> {
        let inner = self.inner.read();
        let job = inner.ingestion_jobs.get(&id).ok_or_else(|| StorageError::not_found("ingestion_job"))?;
        Self::authorize(principal, job)?;
        Ok(job.clone())
    }

    async fn list_ingestion_jobs(
        &self,
        principal: &Principal,
        vector_store_id: Option<Uuid>,
        spec: &QuerySpec,
    ) -> StorageResult<Page<IngestionJob>> {
        let inner = self.inner.read();
        let mut items: Vec<IngestionJob> = inner
            .ingestion_jobs
            .values()
            .filter(|j| vector_store_id.map_or(true, |vsid| j.vector_store_id == vsid))
            .filter(|j| j.owner_id == principal.user_id || principal.has_scope(Scope::System))
            .cloned()
            .collect();

        items.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        let total = items.len();
        let offset = spec.offset.unwrap_or(0);
        let limit = spec.limit.unwrap_or(20);
        let items = items.into_iter().skip(offset).take(limit).collect();

        Ok(Page { items, total, offset, limit })
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_principal() -> Principal {
        Principal {
            user_id: "user-123".into(),
            tenant_id: None,
            scopes: vec![Scope::Read, Scope::Write],
        }
    }

    #[tokio::test]
    async fn test_thread_crud() {
        let backend = MemoryBackend::new();
        let principal = test_principal();

        let thread = Thread {
            id: Uuid::new_v4(),
            owner_id: principal.user_id.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            title: Some("Test Thread".into()),
            metadata: serde_json::Value::Null,
        };

        // Create
        let created = backend.create_thread(&principal, &thread).await.unwrap();
        assert_eq!(created.id, thread.id);

        // Get
        let fetched = backend.get_thread(&principal, thread.id).await.unwrap();
        assert_eq!(fetched.title, thread.title);

        // List
        let spec = QuerySpec::default();
        let page = backend.list_threads(&principal, &spec).await.unwrap();
        assert_eq!(page.items.len(), 1);

        // Delete
        backend.delete_thread(&principal, thread.id).await.unwrap();
        assert!(backend.get_thread(&principal, thread.id).await.is_err());
    }

    #[tokio::test]
    async fn test_unauthorized_access() {
        let backend = MemoryBackend::new();
        let owner = test_principal();
        let other = Principal {
            user_id: "other-user".into(),
            tenant_id: None,
            scopes: vec![Scope::Read],
        };

        let thread = Thread {
            id: Uuid::new_v4(),
            owner_id: owner.user_id.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            title: None,
            metadata: serde_json::Value::Null,
        };

        backend.create_thread(&owner, &thread).await.unwrap();

        let result = backend.get_thread(&other, thread.id).await;
        assert!(matches!(result, Err(StorageError::Unauthorized(_))));
    }

    #[tokio::test]
    async fn test_run_event_sourcing() {
        let backend = MemoryBackend::new();
        let principal = test_principal();

        let run = Run {
            id: Uuid::new_v4(),
            owner_id: principal.user_id.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thread_id: Uuid::new_v4(),
            assistant_id: None,
            status: RunStatus::Queued,
            model: "test".into(),
            instructions: None,
            tools: vec![],
            temperature: None,
            max_prompt_tokens: None,
            max_completion_tokens: None,
            metadata: serde_json::Value::Null,
            started_at: None,
            completed_at: None,
            expired_at: None,
            failed_at: None,
            last_error: None,
        };

        backend.create_run(&principal, &run).await.unwrap();

        let event = RunEvent {
            id: Uuid::new_v4(),
            run_id: run.id,
            created_at: Utc::now(),
            event_type: RunEventType::RunCreated,
        };

        backend.append_run_event(&principal, &event).await.unwrap();

        let events = backend.get_run_events(&principal, run.id).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_vector_search() {
        let backend = MemoryBackend::new();
        let principal = test_principal();

        let store = VectorStore {
            id: Uuid::new_v4(),
            owner_id: principal.user_id.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            name: Some("test-store".into()),
            bytes: 0,
            file_counts: FileCounts::default(),
            status: VectorStoreStatus::Completed,
            expires_after: None,
            metadata: serde_json::Value::Null,
        };
        backend.create_vector_store(&principal, &store).await.unwrap();

        let chunk = VectorChunk {
            id: Uuid::new_v4(),
            owner_id: principal.user_id.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            vector_store_id: store.id,
            file_id: Uuid::new_v4(),
            embedding_model: "test".into(),
            dimension: 3,
            embedding: vec![1.0, 0.0, 0.0],
            content: "hello world".into(),
            content_type: ChunkContentType::Text,
            metadata: serde_json::Value::Null,
            index: 0,
        };
        backend.create_vector_chunks(&principal, &[chunk]).await.unwrap();

        let query = VectorQuery {
            vector_store_id: store.id,
            embedding: vec![1.0, 0.0, 0.0],
            top_k: 5,
            filter: None,
            min_score: None,
        };

        let results = backend.vector_search(&principal, &query).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "hello world");
    }
}
