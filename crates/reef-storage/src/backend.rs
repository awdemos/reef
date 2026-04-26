use async_trait::async_trait;
use uuid::Uuid;

use crate::{
    error::StorageResult,
    models::*,
    query::{Page, QuerySpec, ScoredChunk, VectorQuery},
};

/// Unified storage contract.
///
/// Every method receives an explicit [`Principal`] so authorization is
/// push-down, not magic (no Supabase RLS assumptions).
///
/// Backends implement tiered capability levels:
/// - Tier 1: CRUD + simple queries (all backends).
/// - Tier 2: Vector search + event sourcing (Supabase today; Turso via sqlite-vec soon).
#[async_trait]
pub trait StorageBackend: Send + Sync + 'static {
    // ------------------------------------------------------------------
    // Health
    // ------------------------------------------------------------------
    async fn ping(&self) -> StorageResult<()>;

    // ------------------------------------------------------------------
    // Tier 1: CRUD + Query
    // ------------------------------------------------------------------
    async fn create_thread(&self, principal: &Principal, thread: &Thread) -> StorageResult<Thread>;
    async fn get_thread(&self, principal: &Principal, id: Uuid) -> StorageResult<Thread>;
    async fn update_thread(&self, principal: &Principal, thread: &Thread) -> StorageResult<Thread>;
    async fn delete_thread(&self, principal: &Principal, id: Uuid) -> StorageResult<()>;
    async fn list_threads(
        &self,
        principal: &Principal,
        spec: &QuerySpec,
    ) -> StorageResult<Page<Thread>>;

    async fn create_message(
        &self,
        principal: &Principal,
        message: &Message,
    ) -> StorageResult<Message>;
    async fn get_message(&self, principal: &Principal, id: Uuid) -> StorageResult<Message>;
    async fn list_messages(
        &self,
        principal: &Principal,
        thread_id: Uuid,
        spec: &QuerySpec,
    ) -> StorageResult<Page<Message>>;

    async fn create_run(&self, principal: &Principal, run: &Run) -> StorageResult<Run>;
    async fn get_run(&self, principal: &Principal, id: Uuid) -> StorageResult<Run>;
    async fn update_run(&self, principal: &Principal, run: &Run) -> StorageResult<Run>;
    async fn list_runs(
        &self,
        principal: &Principal,
        thread_id: Uuid,
        spec: &QuerySpec,
    ) -> StorageResult<Page<Run>>;

    async fn create_assistant(
        &self,
        principal: &Principal,
        assistant: &Assistant,
    ) -> StorageResult<Assistant>;
    async fn get_assistant(&self, principal: &Principal, id: Uuid) -> StorageResult<Assistant>;
    async fn update_assistant(
        &self,
        principal: &Principal,
        assistant: &Assistant,
    ) -> StorageResult<Assistant>;
    async fn delete_assistant(&self, principal: &Principal, id: Uuid) -> StorageResult<()>;
    async fn list_assistants(
        &self,
        principal: &Principal,
        spec: &QuerySpec,
    ) -> StorageResult<Page<Assistant>>;

    async fn create_vector_store(
        &self,
        principal: &Principal,
        store: &VectorStore,
    ) -> StorageResult<VectorStore>;
    async fn get_vector_store(&self, principal: &Principal, id: Uuid) -> StorageResult<VectorStore>;
    async fn delete_vector_store(&self, principal: &Principal, id: Uuid) -> StorageResult<()>;
    async fn list_vector_stores(
        &self,
        principal: &Principal,
        spec: &QuerySpec,
    ) -> StorageResult<Page<VectorStore>>;

    async fn create_file(
        &self,
        principal: &Principal,
        file: &FileObject,
    ) -> StorageResult<FileObject>;
    async fn get_file(&self, principal: &Principal, id: Uuid) -> StorageResult<FileObject>;
    async fn delete_file(&self, principal: &Principal, id: Uuid) -> StorageResult<()>;
    async fn list_files(
        &self,
        principal: &Principal,
        spec: &QuerySpec,
    ) -> StorageResult<Page<FileObject>>;

    async fn create_api_key(
        &self,
        principal: &Principal,
        key: &ApiKey,
    ) -> StorageResult<ApiKey>;
    async fn get_api_key_by_hash(
        &self,
        principal: &Principal,
        hash: &str,
    ) -> StorageResult<ApiKey>;
    async fn list_api_keys(
        &self,
        principal: &Principal,
        spec: &QuerySpec,
    ) -> StorageResult<Page<ApiKey>>;
    async fn delete_api_key(&self, principal: &Principal, id: Uuid) -> StorageResult<()>;

    // ------------------------------------------------------------------
    // Tier 2: Vector Search
    // ------------------------------------------------------------------
    async fn vector_search(
        &self,
        principal: &Principal,
        query: &VectorQuery,
    ) -> StorageResult<Vec<ScoredChunk>>;

    async fn create_vector_chunks(
        &self,
        principal: &Principal,
        chunks: &[VectorChunk],
    ) -> StorageResult<Vec<VectorChunk>>;

    async fn delete_vector_chunks_by_file(
        &self,
        principal: &Principal,
        vector_store_id: Uuid,
        file_id: Uuid,
    ) -> StorageResult<u64>;

    // ------------------------------------------------------------------
    // Tier 2: Event Sourcing (Run append-only log)
    // ------------------------------------------------------------------
    async fn append_run_event(
        &self,
        principal: &Principal,
        event: &RunEvent,
    ) -> StorageResult<RunEvent>;

    async fn get_run_events(
        &self,
        principal: &Principal,
        run_id: Uuid,
    ) -> StorageResult<Vec<RunEvent>>;

    // ------------------------------------------------------------------
    // Tier 2: Ingestion Jobs
    // ------------------------------------------------------------------
    async fn create_ingestion_job(
        &self,
        principal: &Principal,
        job: &IngestionJob,
    ) -> StorageResult<IngestionJob>;

    async fn update_ingestion_job(
        &self,
        principal: &Principal,
        job: &IngestionJob,
    ) -> StorageResult<IngestionJob>;

    async fn get_ingestion_job(
        &self,
        principal: &Principal,
        id: Uuid,
    ) -> StorageResult<IngestionJob>;

    async fn list_ingestion_jobs(
        &self,
        principal: &Principal,
        vector_store_id: Option<Uuid>,
        spec: &QuerySpec,
    ) -> StorageResult<Page<IngestionJob>>;
}

/// Helper to stamp `updated_at` before a write.
pub fn touch<E: super::models::Entity>(_entity: &mut E) {
    // This is a no-op in the trait because we can't generically mutate
    // the struct fields without a macro. Each impl should set updated_at.
}
