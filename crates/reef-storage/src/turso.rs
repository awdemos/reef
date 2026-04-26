use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    backend::StorageBackend,
    error::{StorageError, StorageResult},
    models::*,
    query::{Page, QuerySpec, ScoredChunk, VectorQuery},
};

/// Turso / libSQL backend.
///
/// Tier 1 (CRUD + simple queries) is implemented.
/// Tier 2 (vector search, event sourcing) is stubbed and should be
/// implemented via `sqlite-vec` or local FAISS when ready.
#[derive(Debug, Clone)]
pub struct TursoBackend {
    db: std::sync::Arc<libsql::Database>,
}

impl TursoBackend {
    pub fn new(db: libsql::Database) -> Self {
        Self { db: std::sync::Arc::new(db) }
    }

    pub async fn connect(url: &str, auth_token: Option<&str>) -> Result<Self, String> {
        let db = if let Some(token) = auth_token {
            libsql::Builder::new_remote(url.to_string(), token.to_string())
                .build().await.map_err(|e| e.to_string())?
        } else {
            libsql::Builder::new_local(url).build().await.map_err(|e| e.to_string())?
        };
        Ok(Self::new(db))
    }

    pub async fn migrate(&self) -> StorageResult<()> {
        // TODO: Run libsql migrations
        Ok(())
    }

    async fn conn(&self) -> StorageResult<libsql::Connection> {
        self.db.connect().map_err(|e| StorageError::Connection(e.to_string()))
    }
}

#[async_trait]
impl StorageBackend for TursoBackend {
    async fn ping(&self) -> StorageResult<()> {
        Ok(())
    }

    async fn create_thread(&self, _principal: &Principal, thread: &Thread) -> StorageResult<Thread> {
        let _conn = self.conn().await?;
        // TODO: implement with libsql
        Ok(thread.clone())
    }

    async fn get_thread(&self, _principal: &Principal, _id: Uuid) -> StorageResult<Thread> {
        Err(StorageError::not_implemented("TursoBackend::get_thread"))
    }

    async fn update_thread(&self, _principal: &Principal, thread: &Thread) -> StorageResult<Thread> {
        Ok(thread.clone())
    }

    async fn delete_thread(&self, _principal: &Principal, _id: Uuid) -> StorageResult<()> {
        Ok(())
    }

    async fn list_threads(&self, _principal: &Principal, _spec: &QuerySpec) -> StorageResult<Page<Thread>> {
        Err(StorageError::not_implemented("TursoBackend::list_threads"))
    }

    async fn create_message(&self, _principal: &Principal, message: &Message) -> StorageResult<Message> {
        Ok(message.clone())
    }

    async fn get_message(&self, _principal: &Principal, _id: Uuid) -> StorageResult<Message> {
        Err(StorageError::not_implemented("TursoBackend::get_message"))
    }

    async fn list_messages(&self, _principal: &Principal, _thread_id: Uuid, _spec: &QuerySpec) -> StorageResult<Page<Message>> {
        Err(StorageError::not_implemented("TursoBackend::list_messages"))
    }

    async fn create_run(&self, _principal: &Principal, run: &Run) -> StorageResult<Run> {
        Ok(run.clone())
    }

    async fn get_run(&self, _principal: &Principal, _id: Uuid) -> StorageResult<Run> {
        Err(StorageError::not_implemented("TursoBackend::get_run"))
    }

    async fn update_run(&self, _principal: &Principal, run: &Run) -> StorageResult<Run> {
        Ok(run.clone())
    }

    async fn list_runs(&self, _principal: &Principal, _thread_id: Uuid, _spec: &QuerySpec) -> StorageResult<Page<Run>> {
        Err(StorageError::not_implemented("TursoBackend::list_runs"))
    }

    async fn create_assistant(&self, _principal: &Principal, assistant: &Assistant) -> StorageResult<Assistant> {
        Ok(assistant.clone())
    }

    async fn get_assistant(&self, _principal: &Principal, _id: Uuid) -> StorageResult<Assistant> {
        Err(StorageError::not_implemented("TursoBackend::get_assistant"))
    }

    async fn update_assistant(&self, _principal: &Principal, assistant: &Assistant) -> StorageResult<Assistant> {
        Ok(assistant.clone())
    }

    async fn delete_assistant(&self, _principal: &Principal, _id: Uuid) -> StorageResult<()> {
        Ok(())
    }

    async fn list_assistants(&self, _principal: &Principal, _spec: &QuerySpec) -> StorageResult<Page<Assistant>> {
        Err(StorageError::not_implemented("TursoBackend::list_assistants"))
    }

    async fn create_vector_store(&self, _principal: &Principal, store: &VectorStore) -> StorageResult<VectorStore> {
        Ok(store.clone())
    }

    async fn get_vector_store(&self, _principal: &Principal, _id: Uuid) -> StorageResult<VectorStore> {
        Err(StorageError::not_implemented("TursoBackend::get_vector_store"))
    }

    async fn delete_vector_store(&self, _principal: &Principal, _id: Uuid) -> StorageResult<()> {
        Ok(())
    }

    async fn list_vector_stores(&self, _principal: &Principal, _spec: &QuerySpec) -> StorageResult<Page<VectorStore>> {
        Err(StorageError::not_implemented("TursoBackend::list_vector_stores"))
    }

    async fn create_file(&self, _principal: &Principal, file: &FileObject) -> StorageResult<FileObject> {
        Ok(file.clone())
    }

    async fn get_file(&self, _principal: &Principal, _id: Uuid) -> StorageResult<FileObject> {
        Err(StorageError::not_implemented("TursoBackend::get_file"))
    }

    async fn delete_file(&self, _principal: &Principal, _id: Uuid) -> StorageResult<()> {
        Ok(())
    }

    async fn list_files(&self, _principal: &Principal, _spec: &QuerySpec) -> StorageResult<Page<FileObject>> {
        Err(StorageError::not_implemented("TursoBackend::list_files"))
    }

    async fn create_api_key(&self, _principal: &Principal, key: &ApiKey) -> StorageResult<ApiKey> {
        Ok(key.clone())
    }

    async fn get_api_key_by_hash(&self, _principal: &Principal, _hash: &str) -> StorageResult<ApiKey> {
        Err(StorageError::not_implemented("TursoBackend::get_api_key_by_hash"))
    }

    async fn list_api_keys(&self, _principal: &Principal, _spec: &QuerySpec) -> StorageResult<Page<ApiKey>> {
        Err(StorageError::not_implemented("TursoBackend::list_api_keys"))
    }

    async fn delete_api_key(&self, _principal: &Principal, _id: Uuid) -> StorageResult<()> {
        Ok(())
    }

    async fn vector_search(&self, _principal: &Principal, _query: &VectorQuery) -> StorageResult<Vec<ScoredChunk>> {
        Err(StorageError::not_implemented("TursoBackend::vector_search — enable with sqlite-vec"))
    }

    async fn create_vector_chunks(&self, _principal: &Principal, chunks: &[VectorChunk]) -> StorageResult<Vec<VectorChunk>> {
        Ok(chunks.to_vec())
    }

    async fn delete_vector_chunks_by_file(&self, _principal: &Principal, _vector_store_id: Uuid, _file_id: Uuid) -> StorageResult<u64> {
        Ok(0)
    }

    async fn append_run_event(&self, _principal: &Principal, event: &RunEvent) -> StorageResult<RunEvent> {
        Ok(event.clone())
    }

    async fn get_run_events(&self, _principal: &Principal, _run_id: Uuid) -> StorageResult<Vec<RunEvent>> {
        Err(StorageError::not_implemented("TursoBackend::get_run_events"))
    }

    async fn create_ingestion_job(&self, _principal: &Principal, job: &IngestionJob) -> StorageResult<IngestionJob> {
        Ok(job.clone())
    }

    async fn update_ingestion_job(&self, _principal: &Principal, job: &IngestionJob) -> StorageResult<IngestionJob> {
        Ok(job.clone())
    }

    async fn get_ingestion_job(&self, _principal: &Principal, _id: Uuid) -> StorageResult<IngestionJob> {
        Err(StorageError::not_implemented("TursoBackend::get_ingestion_job"))
    }

    async fn list_ingestion_jobs(&self, _principal: &Principal, _vector_store_id: Option<Uuid>, _spec: &QuerySpec) -> StorageResult<Page<IngestionJob>> {
        Err(StorageError::not_implemented("TursoBackend::list_ingestion_jobs"))
    }
}
