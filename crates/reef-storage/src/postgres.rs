use async_trait::async_trait;
use chrono::Utc;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::{
    backend::StorageBackend,
    error::{StorageError, StorageResult},
    models::*,
    query::{Direction, FilterOp, Page, QuerySpec, ScoredChunk, VectorQuery},
};

/// PostgreSQL + pgvector backend.
#[derive(Debug, Clone)]
pub struct PostgresBackend {
    pool: PgPool,
}

impl PostgresBackend {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> StorageResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(format!("migration failed: {}", e)))?;
        Ok(())
    }

    fn apply_pagination<'a>(
        &self,
        mut query: String,
        spec: &QuerySpec,
    ) -> String {
        if let Some(order_by) = &spec.order_by {
            let dir = match spec.direction {
                Direction::Asc => "ASC",
                Direction::Desc => "DESC",
            };
            query.push_str(&format!(" ORDER BY {} {}", order_by, dir));
        }

        let limit = spec.limit.unwrap_or(20);
        let offset = spec.offset.unwrap_or(0);
        query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));
        query
    }

    fn build_where(&self, spec: &QuerySpec) -> (String, Vec<serde_json::Value>) {
        let mut clauses = Vec::new();
        let mut params = Vec::new();

        for filter in &spec.filters {
            let op = match filter.op {
                FilterOp::Eq => "=",
                FilterOp::Neq => "!=",
                FilterOp::Gt => ">",
                FilterOp::Gte => ">=",
                FilterOp::Lt => "<",
                FilterOp::Lte => "<=",
                FilterOp::Like => "LIKE",
                FilterOp::In => "IN",
                FilterOp::IsNull => {
                    clauses.push(format!("{} IS NULL", filter.field));
                    continue;
                }
            };
            clauses.push(format!("{} {} ${}", filter.field, op, params.len() + 1));
            params.push(filter.value.clone());
        }

        if clauses.is_empty() {
            (String::new(), params)
        } else {
            (format!("WHERE {}", clauses.join(" AND ")), params)
        }
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    async fn ping(&self) -> StorageResult<()> {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await.map_err(|e| {
            StorageError::Connection(e.to_string())
        })?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Threads
    // ------------------------------------------------------------------
    async fn create_thread(&self, _principal: &Principal, thread: &Thread) -> StorageResult<Thread> {
        sqlx::query(
            r#"
            INSERT INTO threads (id, owner_id, created_at, updated_at, title, metadata)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(thread.id)
        .bind(&thread.owner_id)
        .bind(thread.created_at)
        .bind(thread.updated_at)
        .bind(&thread.title)
        .bind(&thread.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(thread.clone())
    }

    async fn get_thread(&self, principal: &Principal, id: Uuid) -> StorageResult<Thread> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, title, metadata FROM threads WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("thread"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let thread = Thread {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            title: row.get("title"),
            metadata: row.get("metadata"),
        };

        if thread.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(thread)
    }

    async fn update_thread(&self, principal: &Principal, thread: &Thread) -> StorageResult<Thread> {
        self.get_thread(principal, thread.id).await?; // auth check

        sqlx::query(
            "UPDATE threads SET updated_at = $1, title = $2, metadata = $3 WHERE id = $4"
        )
        .bind(thread.updated_at)
        .bind(&thread.title)
        .bind(&thread.metadata)
        .bind(thread.id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(thread.clone())
    }

    async fn delete_thread(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        self.get_thread(principal, id).await?; // auth check

        sqlx::query("DELETE FROM threads WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_threads(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<Thread>> {
        let base = "SELECT id, owner_id, created_at, updated_at, title, metadata FROM threads WHERE owner_id = $1".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM threads WHERE owner_id = $1")
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<Thread> = rows.into_iter().map(|row| Thread {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            title: row.get("title"),
            metadata: row.get("metadata"),
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    // ------------------------------------------------------------------
    // Messages
    // ------------------------------------------------------------------
    async fn create_message(&self, _principal: &Principal, message: &Message) -> StorageResult<Message> {
        sqlx::query(
            r#"
            INSERT INTO messages (id, owner_id, created_at, updated_at, thread_id, run_id, role, content, annotations, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(message.id)
        .bind(&message.owner_id)
        .bind(message.created_at)
        .bind(message.updated_at)
        .bind(message.thread_id)
        .bind(message.run_id)
        .bind(&message.role)
        .bind(sqlx::types::Json(&message.content))
        .bind(sqlx::types::Json(&message.annotations))
        .bind(&message.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(message.clone())
    }

    async fn get_message(&self, principal: &Principal, id: Uuid) -> StorageResult<Message> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, thread_id, run_id, role, content, annotations, metadata FROM messages WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("message"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let msg = Message {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            thread_id: row.get("thread_id"),
            run_id: row.get("run_id"),
            role: row.get("role"),
            content: serde_json::from_value(row.get("content")).unwrap_or_default(),
            annotations: serde_json::from_value(row.get("annotations")).unwrap_or_default(),
            metadata: row.get("metadata"),
        };

        if msg.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(msg)
    }

    async fn list_messages(&self, principal: &Principal, thread_id: Uuid, spec: &QuerySpec) -> StorageResult<Page<Message>> {
        let base = "SELECT id, owner_id, created_at, updated_at, thread_id, run_id, role, content, annotations, metadata FROM messages WHERE thread_id = $1 AND owner_id = $2".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(thread_id)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM messages WHERE thread_id = $1 AND owner_id = $2")
            .bind(thread_id)
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<Message> = rows.into_iter().map(|row| Message {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            thread_id: row.get("thread_id"),
            run_id: row.get("run_id"),
            role: row.get("role"),
            content: serde_json::from_value(row.get("content")).unwrap_or_default(),
            annotations: serde_json::from_value(row.get("annotations")).unwrap_or_default(),
            metadata: row.get("metadata"),
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    // ------------------------------------------------------------------
    // Runs
    // ------------------------------------------------------------------
    async fn create_run(&self, _principal: &Principal, run: &Run) -> StorageResult<Run> {
        sqlx::query(
            r#"
            INSERT INTO runs (id, owner_id, created_at, updated_at, thread_id, assistant_id, status, model, instructions, tools, temperature, max_prompt_tokens, max_completion_tokens, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#
        )
        .bind(run.id)
        .bind(&run.owner_id)
        .bind(run.created_at)
        .bind(run.updated_at)
        .bind(run.thread_id)
        .bind(run.assistant_id)
        .bind(serde_json::to_string(&run.status).unwrap_or_default().trim_matches('"'))
        .bind(&run.model)
        .bind(&run.instructions)
        .bind(sqlx::types::Json(&run.tools))
        .bind(run.temperature)
        .bind(run.max_prompt_tokens)
        .bind(run.max_completion_tokens)
        .bind(&run.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(run.clone())
    }

    async fn get_run(&self, principal: &Principal, id: Uuid) -> StorageResult<Run> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, thread_id, assistant_id, status, model, instructions, tools, temperature, max_prompt_tokens, max_completion_tokens, metadata, started_at, completed_at, expired_at, failed_at, last_error FROM runs WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("run"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "queued" => RunStatus::Queued,
            "in_progress" => RunStatus::InProgress,
            "requires_action" => RunStatus::RequiresAction,
            "cancelling" => RunStatus::Cancelling,
            "cancelled" => RunStatus::Cancelled,
            "failed" => RunStatus::Failed,
            "completed" => RunStatus::Completed,
            "expired" => RunStatus::Expired,
            _ => RunStatus::Queued,
        };

        let run = Run {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            thread_id: row.get("thread_id"),
            assistant_id: row.get("assistant_id"),
            status,
            model: row.get("model"),
            instructions: row.get("instructions"),
            tools: serde_json::from_value(row.get("tools")).unwrap_or_default(),
            temperature: row.get("temperature"),
            max_prompt_tokens: row.get("max_prompt_tokens"),
            max_completion_tokens: row.get("max_completion_tokens"),
            metadata: row.get("metadata"),
            started_at: row.get("started_at"),
            completed_at: row.get("completed_at"),
            expired_at: row.get("expired_at"),
            failed_at: row.get("failed_at"),
            last_error: serde_json::from_value(row.get("last_error")).unwrap_or_default(),
        };

        if run.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(run)
    }

    async fn update_run(&self, principal: &Principal, run: &Run) -> StorageResult<Run> {
        self.get_run(principal, run.id).await?; // auth check

        sqlx::query(
            r#"
            UPDATE runs SET
                updated_at = $1,
                status = $2,
                model = $3,
                instructions = $4,
                tools = $5,
                temperature = $6,
                max_prompt_tokens = $7,
                max_completion_tokens = $8,
                metadata = $9,
                started_at = $10,
                completed_at = $11,
                expired_at = $12,
                failed_at = $13,
                last_error = $14
            WHERE id = $15
            "#
        )
        .bind(run.updated_at)
        .bind(serde_json::to_string(&run.status).unwrap_or_default().trim_matches('"'))
        .bind(&run.model)
        .bind(&run.instructions)
        .bind(sqlx::types::Json(&run.tools))
        .bind(run.temperature)
        .bind(run.max_prompt_tokens)
        .bind(run.max_completion_tokens)
        .bind(&run.metadata)
        .bind(run.started_at)
        .bind(run.completed_at)
        .bind(run.expired_at)
        .bind(run.failed_at)
        .bind(sqlx::types::Json(&run.last_error))
        .bind(run.id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(run.clone())
    }

    async fn list_runs(&self, principal: &Principal, thread_id: Uuid, spec: &QuerySpec) -> StorageResult<Page<Run>> {
        let base = "SELECT id, owner_id, created_at, updated_at, thread_id, assistant_id, status, model, instructions, tools, temperature, max_prompt_tokens, max_completion_tokens, metadata, started_at, completed_at, expired_at, failed_at, last_error FROM runs WHERE thread_id = $1 AND owner_id = $2".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(thread_id)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM runs WHERE thread_id = $1 AND owner_id = $2")
            .bind(thread_id)
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<Run> = rows.into_iter().map(|row| {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "queued" => RunStatus::Queued,
                "in_progress" => RunStatus::InProgress,
                "requires_action" => RunStatus::RequiresAction,
                "cancelling" => RunStatus::Cancelling,
                "cancelled" => RunStatus::Cancelled,
                "failed" => RunStatus::Failed,
                "completed" => RunStatus::Completed,
                "expired" => RunStatus::Expired,
                _ => RunStatus::Queued,
            };

            Run {
                id: row.get("id"),
                owner_id: row.get("owner_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                thread_id: row.get("thread_id"),
                assistant_id: row.get("assistant_id"),
                status,
                model: row.get("model"),
                instructions: row.get("instructions"),
                tools: serde_json::from_value(row.get("tools")).unwrap_or_default(),
                temperature: row.get("temperature"),
                max_prompt_tokens: row.get("max_prompt_tokens"),
                max_completion_tokens: row.get("max_completion_tokens"),
                metadata: row.get("metadata"),
                started_at: row.get("started_at"),
                completed_at: row.get("completed_at"),
                expired_at: row.get("expired_at"),
                failed_at: row.get("failed_at"),
                last_error: serde_json::from_value(row.get("last_error")).unwrap_or_default(),
            }
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    // ------------------------------------------------------------------
    // Assistants
    // ------------------------------------------------------------------
    async fn create_assistant(&self, _principal: &Principal, assistant: &Assistant) -> StorageResult<Assistant> {
        sqlx::query(
            r#"
            INSERT INTO assistants (id, owner_id, created_at, updated_at, name, description, model, instructions, tools, tool_resources, metadata, temperature, top_p, response_format)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#
        )
        .bind(assistant.id)
        .bind(&assistant.owner_id)
        .bind(assistant.created_at)
        .bind(assistant.updated_at)
        .bind(&assistant.name)
        .bind(&assistant.description)
        .bind(&assistant.model)
        .bind(&assistant.instructions)
        .bind(sqlx::types::Json(&assistant.tools))
        .bind(&assistant.tool_resources)
        .bind(&assistant.metadata)
        .bind(assistant.temperature)
        .bind(assistant.top_p)
        .bind(&assistant.response_format)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(assistant.clone())
    }

    async fn get_assistant(&self, principal: &Principal, id: Uuid) -> StorageResult<Assistant> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, name, description, model, instructions, tools, tool_resources, metadata, temperature, top_p, response_format FROM assistants WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("assistant"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let assistant = Assistant {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            name: row.get("name"),
            description: row.get("description"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            tools: serde_json::from_value(row.get("tools")).unwrap_or_default(),
            tool_resources: row.get("tool_resources"),
            metadata: row.get("metadata"),
            temperature: row.get("temperature"),
            top_p: row.get("top_p"),
            response_format: row.get("response_format"),
        };

        if assistant.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(assistant)
    }

    async fn update_assistant(&self, principal: &Principal, assistant: &Assistant) -> StorageResult<Assistant> {
        self.get_assistant(principal, assistant.id).await?; // auth check

        sqlx::query(
            "UPDATE assistants SET updated_at = $1, name = $2, description = $3, model = $4, instructions = $5, tools = $6, tool_resources = $7, metadata = $8, temperature = $9, top_p = $10, response_format = $11 WHERE id = $12"
        )
        .bind(assistant.updated_at)
        .bind(&assistant.name)
        .bind(&assistant.description)
        .bind(&assistant.model)
        .bind(&assistant.instructions)
        .bind(sqlx::types::Json(&assistant.tools))
        .bind(&assistant.tool_resources)
        .bind(&assistant.metadata)
        .bind(assistant.temperature)
        .bind(assistant.top_p)
        .bind(&assistant.response_format)
        .bind(assistant.id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(assistant.clone())
    }

    async fn delete_assistant(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        self.get_assistant(principal, id).await?; // auth check

        sqlx::query("DELETE FROM assistants WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_assistants(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<Assistant>> {
        let base = "SELECT id, owner_id, created_at, updated_at, name, description, model, instructions, tools, tool_resources, metadata, temperature, top_p, response_format FROM assistants WHERE owner_id = $1".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM assistants WHERE owner_id = $1")
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<Assistant> = rows.into_iter().map(|row| Assistant {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            name: row.get("name"),
            description: row.get("description"),
            model: row.get("model"),
            instructions: row.get("instructions"),
            tools: serde_json::from_value(row.get("tools")).unwrap_or_default(),
            tool_resources: row.get("tool_resources"),
            metadata: row.get("metadata"),
            temperature: row.get("temperature"),
            top_p: row.get("top_p"),
            response_format: row.get("response_format"),
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    // ------------------------------------------------------------------
    // Vector Stores
    // ------------------------------------------------------------------
    async fn create_vector_store(&self, _principal: &Principal, store: &VectorStore) -> StorageResult<VectorStore> {
        sqlx::query(
            r#"
            INSERT INTO vector_stores (id, owner_id, created_at, updated_at, name, bytes, file_counts, status, expires_after, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(store.id)
        .bind(&store.owner_id)
        .bind(store.created_at)
        .bind(store.updated_at)
        .bind(&store.name)
        .bind(store.bytes)
        .bind(sqlx::types::Json(&store.file_counts))
        .bind(serde_json::to_string(&store.status).unwrap_or_default().trim_matches('"'))
        .bind(&store.expires_after)
        .bind(&store.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(store.clone())
    }

    async fn get_vector_store(&self, principal: &Principal, id: Uuid) -> StorageResult<VectorStore> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, name, bytes, file_counts, status, expires_after, metadata FROM vector_stores WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("vector_store"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "in_progress" => VectorStoreStatus::InProgress,
            "completed" => VectorStoreStatus::Completed,
            "failed" => VectorStoreStatus::Failed,
            _ => VectorStoreStatus::InProgress,
        };

        let store = VectorStore {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            name: row.get("name"),
            bytes: row.get("bytes"),
            file_counts: serde_json::from_value(row.get("file_counts")).unwrap_or_default(),
            status,
            expires_after: row.get("expires_after"),
            metadata: row.get("metadata"),
        };

        if store.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(store)
    }

    async fn delete_vector_store(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        self.get_vector_store(principal, id).await?; // auth check

        sqlx::query("DELETE FROM vector_stores WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_vector_stores(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<VectorStore>> {
        let base = "SELECT id, owner_id, created_at, updated_at, name, bytes, file_counts, status, expires_after, metadata FROM vector_stores WHERE owner_id = $1".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM vector_stores WHERE owner_id = $1")
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<VectorStore> = rows.into_iter().map(|row| {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "in_progress" => VectorStoreStatus::InProgress,
                "completed" => VectorStoreStatus::Completed,
                "failed" => VectorStoreStatus::Failed,
                _ => VectorStoreStatus::InProgress,
            };

            VectorStore {
                id: row.get("id"),
                owner_id: row.get("owner_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                name: row.get("name"),
                bytes: row.get("bytes"),
                file_counts: serde_json::from_value(row.get("file_counts")).unwrap_or_default(),
                status,
                expires_after: row.get("expires_after"),
                metadata: row.get("metadata"),
            }
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    // ------------------------------------------------------------------
    // Files
    // ------------------------------------------------------------------
    async fn create_file(&self, _principal: &Principal, file: &FileObject) -> StorageResult<FileObject> {
        sqlx::query(
            r#"
            INSERT INTO file_objects (id, owner_id, created_at, updated_at, bytes, filename, purpose, status, status_details, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(file.id)
        .bind(&file.owner_id)
        .bind(file.created_at)
        .bind(file.updated_at)
        .bind(file.bytes)
        .bind(&file.filename)
        .bind(&file.purpose)
        .bind(serde_json::to_string(&file.status).unwrap_or_default().trim_matches('"'))
        .bind(&file.status_details)
        .bind(&file.metadata)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(file.clone())
    }

    async fn get_file(&self, principal: &Principal, id: Uuid) -> StorageResult<FileObject> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, bytes, filename, purpose, status, status_details, metadata FROM file_objects WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("file"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "uploaded" => FileStatus::Uploaded,
            "processed" => FileStatus::Processed,
            "error" => FileStatus::Error,
            _ => FileStatus::Uploaded,
        };

        let file = FileObject {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            bytes: row.get("bytes"),
            filename: row.get("filename"),
            purpose: row.get("purpose"),
            status,
            status_details: row.get("status_details"),
            metadata: row.get("metadata"),
        };

        if file.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(file)
    }

    async fn delete_file(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        self.get_file(principal, id).await?; // auth check

        sqlx::query("DELETE FROM file_objects WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    async fn list_files(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<FileObject>> {
        let base = "SELECT id, owner_id, created_at, updated_at, bytes, filename, purpose, status, status_details, metadata FROM file_objects WHERE owner_id = $1".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM file_objects WHERE owner_id = $1")
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<FileObject> = rows.into_iter().map(|row| {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "uploaded" => FileStatus::Uploaded,
                "processed" => FileStatus::Processed,
                "error" => FileStatus::Error,
                _ => FileStatus::Uploaded,
            };

            FileObject {
                id: row.get("id"),
                owner_id: row.get("owner_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                bytes: row.get("bytes"),
                filename: row.get("filename"),
                purpose: row.get("purpose"),
                status,
                status_details: row.get("status_details"),
                metadata: row.get("metadata"),
            }
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    // ------------------------------------------------------------------
    // API Keys
    // ------------------------------------------------------------------
    async fn create_api_key(&self, _principal: &Principal, key: &ApiKey) -> StorageResult<ApiKey> {
        sqlx::query(
            r#"
            INSERT INTO api_keys (id, owner_id, created_at, updated_at, key_hash, key_preview, name, scopes, expires_at, last_used_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(key.id)
        .bind(&key.owner_id)
        .bind(key.created_at)
        .bind(key.updated_at)
        .bind(&key.key_hash)
        .bind(&key.key_preview)
        .bind(&key.name)
        .bind(sqlx::types::Json(&key.scopes))
        .bind(key.expires_at)
        .bind(key.last_used_at)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(key.clone())
    }

    async fn get_api_key_by_hash(&self, _principal: &Principal, hash: &str) -> StorageResult<ApiKey> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, key_hash, key_preview, name, scopes, expires_at, last_used_at FROM api_keys WHERE key_hash = $1"
        )
        .bind(hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("api_key"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        Ok(ApiKey {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            key_hash: row.get("key_hash"),
            key_preview: row.get("key_preview"),
            name: row.get("name"),
            scopes: serde_json::from_value(row.get("scopes")).unwrap_or_default(),
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
        })
    }

    async fn list_api_keys(&self, principal: &Principal, spec: &QuerySpec) -> StorageResult<Page<ApiKey>> {
        let base = "SELECT id, owner_id, created_at, updated_at, key_hash, key_preview, name, scopes, expires_at, last_used_at FROM api_keys WHERE owner_id = $1".to_string();
        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .bind(&principal.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_row = sqlx::query("SELECT COUNT(*) as c FROM api_keys WHERE owner_id = $1")
            .bind(&principal.user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<ApiKey> = rows.into_iter().map(|row| ApiKey {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            key_hash: row.get("key_hash"),
            key_preview: row.get("key_preview"),
            name: row.get("name"),
            scopes: serde_json::from_value(row.get("scopes")).unwrap_or_default(),
            expires_at: row.get("expires_at"),
            last_used_at: row.get("last_used_at"),
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }

    async fn delete_api_key(&self, principal: &Principal, id: Uuid) -> StorageResult<()> {
        let row = sqlx::query("SELECT owner_id FROM api_keys WHERE id = $1")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| match e {
                sqlx::Error::RowNotFound => StorageError::not_found("api_key"),
                _ => StorageError::Internal(e.to_string()),
            })?;

        let owner_id: String = row.get("owner_id");
        if owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        sqlx::query("DELETE FROM api_keys WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(())
    }

    // ------------------------------------------------------------------
    // Tier 2: Vector Search
    // ------------------------------------------------------------------
    async fn vector_search(&self, principal: &Principal, query: &VectorQuery) -> StorageResult<Vec<ScoredChunk>> {
        self.get_vector_store(principal, query.vector_store_id).await?; // auth check

        let rows = sqlx::query(
            "SELECT id, vector_store_id, file_id, content, content_type, metadata, idx, similarity FROM match_vectors($1::vector, $2, $3)"
        )
        .bind(&query.embedding)
        .bind(query.vector_store_id)
        .bind(query.top_k as i32)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Vector(e.to_string()))?;

        let results: Vec<ScoredChunk> = rows.into_iter().map(|row| {
            let content_type_str: String = row.get("content_type");
            let content_type = match content_type_str.as_str() {
                "table" => ChunkContentType::Table,
                "image_caption" => ChunkContentType::ImageCaption,
                "audio_transcript" => ChunkContentType::AudioTranscript,
                _ => ChunkContentType::Text,
            };

            ScoredChunk {
                chunk_id: row.get("id"),
                vector_store_id: row.get("vector_store_id"),
                file_id: row.get("file_id"),
                score: row.get("similarity"),
                content: serde_json::from_value(row.get("content")).unwrap_or_default(),
                content_type,
                metadata: row.get("metadata"),
                index: row.get::<i32, _>("idx") as usize,
            }
        }).collect();

        Ok(results)
    }

    async fn create_vector_chunks(&self, _principal: &Principal, chunks: &[VectorChunk]) -> StorageResult<Vec<VectorChunk>> {
        for chunk in chunks {
            sqlx::query(
                r#"
                INSERT INTO vector_content (id, owner_id, created_at, updated_at, vector_store_id, file_id, embedding_model, dimension, embedding, content, content_type, metadata, idx)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::vector, $10, $11, $12, $13)
                "#
            )
            .bind(chunk.id)
            .bind(&chunk.owner_id)
            .bind(chunk.created_at)
            .bind(chunk.updated_at)
            .bind(chunk.vector_store_id)
            .bind(chunk.file_id)
            .bind(&chunk.embedding_model)
            .bind(chunk.dimension as i32)
            .bind(&chunk.embedding)
            .bind(&chunk.content)
            .bind(serde_json::to_string(&chunk.content_type).unwrap_or_default().trim_matches('"'))
            .bind(sqlx::types::Json(&chunk.metadata))
            .bind(chunk.index as i32)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Vector(e.to_string()))?;
        }

        Ok(chunks.to_vec())
    }

    async fn delete_vector_chunks_by_file(&self, _principal: &Principal, _vector_store_id: Uuid, file_id: Uuid) -> StorageResult<u64> {
        let result = sqlx::query("DELETE FROM vector_content WHERE file_id = $1")
            .bind(file_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StorageError::Vector(e.to_string()))?;

        Ok(result.rows_affected())
    }

    // ------------------------------------------------------------------
    // Tier 2: Event Sourcing
    // ------------------------------------------------------------------
    async fn append_run_event(&self, _principal: &Principal, event: &RunEvent) -> StorageResult<RunEvent> {
        sqlx::query(
            r#"
            INSERT INTO run_events (id, run_id, created_at, event_type, payload)
            VALUES ($1, $2, $3, $4, $5)
            "#
        )
        .bind(event.id)
        .bind(event.run_id)
        .bind(event.created_at)
        .bind(serde_json::to_string(&event.event_type).unwrap_or_default())
        .bind(serde_json::to_value(&event.event_type).unwrap_or_default())
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(event.clone())
    }

    async fn get_run_events(&self, principal: &Principal, run_id: Uuid) -> StorageResult<Vec<RunEvent>> {
        self.get_run(principal, run_id).await?; // auth check

        let rows = sqlx::query(
            "SELECT id, run_id, created_at, event_type, payload FROM run_events WHERE run_id = $1 ORDER BY created_at ASC"
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        let events: Vec<RunEvent> = rows.into_iter().map(|row| {
            let payload: serde_json::Value = row.get("payload");
            let event_type: RunEventType = serde_json::from_value(payload).unwrap_or(RunEventType::RunCreated);

            RunEvent {
                id: row.get("id"),
                run_id: row.get("run_id"),
                created_at: row.get("created_at"),
                event_type,
            }
        }).collect();

        Ok(events)
    }

    // ------------------------------------------------------------------
    // Tier 2: Ingestion Jobs
    // ------------------------------------------------------------------
    async fn create_ingestion_job(&self, _principal: &Principal, job: &IngestionJob) -> StorageResult<IngestionJob> {
        sqlx::query(
            r#"
            INSERT INTO ingestion_jobs (id, owner_id, created_at, updated_at, vector_store_id, file_id, status, stage, error, progress_percent)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#
        )
        .bind(job.id)
        .bind(&job.owner_id)
        .bind(job.created_at)
        .bind(job.updated_at)
        .bind(job.vector_store_id)
        .bind(job.file_id)
        .bind(serde_json::to_string(&job.status).unwrap_or_default().trim_matches('"'))
        .bind(serde_json::to_string(&job.stage).unwrap_or_default().trim_matches('"'))
        .bind(&job.error)
        .bind(job.progress_percent)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(job.clone())
    }

    async fn update_ingestion_job(&self, principal: &Principal, job: &IngestionJob) -> StorageResult<IngestionJob> {
        self.get_ingestion_job(principal, job.id).await?; // auth check

        sqlx::query(
            "UPDATE ingestion_jobs SET updated_at = $1, status = $2, stage = $3, error = $4, progress_percent = $5 WHERE id = $6"
        )
        .bind(job.updated_at)
        .bind(serde_json::to_string(&job.status).unwrap_or_default().trim_matches('"'))
        .bind(serde_json::to_string(&job.stage).unwrap_or_default().trim_matches('"'))
        .bind(&job.error)
        .bind(job.progress_percent)
        .bind(job.id)
        .execute(&self.pool)
        .await
        .map_err(|e| StorageError::Internal(e.to_string()))?;

        Ok(job.clone())
    }

    async fn get_ingestion_job(&self, principal: &Principal, id: Uuid) -> StorageResult<IngestionJob> {
        let row = sqlx::query(
            "SELECT id, owner_id, created_at, updated_at, vector_store_id, file_id, status, stage, error, progress_percent FROM ingestion_jobs WHERE id = $1"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => StorageError::not_found("ingestion_job"),
            _ => StorageError::Internal(e.to_string()),
        })?;

        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "pending" => IngestionStatus::Pending,
            "running" => IngestionStatus::Running,
            "completed" => IngestionStatus::Completed,
            "failed" => IngestionStatus::Failed,
            "cancelled" => IngestionStatus::Cancelled,
            _ => IngestionStatus::Pending,
        };

        let stage_str: String = row.get("stage");
        let stage = match stage_str.as_str() {
            "download" => IngestionStage::Download,
            "parse" => IngestionStage::Parse,
            "chunk" => IngestionStage::Chunk,
            "embed" => IngestionStage::Embed,
            "index" => IngestionStage::Index,
            _ => IngestionStage::Download,
        };

        let job = IngestionJob {
            id: row.get("id"),
            owner_id: row.get("owner_id"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            vector_store_id: row.get("vector_store_id"),
            file_id: row.get("file_id"),
            status,
            stage,
            error: row.get("error"),
            progress_percent: row.get("progress_percent"),
        };

        if job.owner_id != principal.user_id && !principal.has_scope(Scope::System) {
            return Err(StorageError::unauthorized("not owner"));
        }

        Ok(job)
    }

    async fn list_ingestion_jobs(&self, principal: &Principal, vector_store_id: Option<Uuid>, spec: &QuerySpec) -> StorageResult<Page<IngestionJob>> {
        let base = if let Some(vsid) = vector_store_id {
            format!("SELECT id, owner_id, created_at, updated_at, vector_store_id, file_id, status, stage, error, progress_percent FROM ingestion_jobs WHERE owner_id = '{}' AND vector_store_id = '{}'", principal.user_id, vsid)
        } else {
            format!("SELECT id, owner_id, created_at, updated_at, vector_store_id, file_id, status, stage, error, progress_percent FROM ingestion_jobs WHERE owner_id = '{}'", principal.user_id)
        };

        let query = self.apply_pagination(base, spec);

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let count_query = if let Some(vsid) = vector_store_id {
            format!("SELECT COUNT(*) as c FROM ingestion_jobs WHERE owner_id = '{}' AND vector_store_id = '{}'", principal.user_id, vsid)
        } else {
            format!("SELECT COUNT(*) as c FROM ingestion_jobs WHERE owner_id = '{}'", principal.user_id)
        };

        let count_row = sqlx::query(&count_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StorageError::Internal(e.to_string()))?;

        let total: i64 = count_row.get("c");

        let items: Vec<IngestionJob> = rows.into_iter().map(|row| {
            let status_str: String = row.get("status");
            let status = match status_str.as_str() {
                "pending" => IngestionStatus::Pending,
                "running" => IngestionStatus::Running,
                "completed" => IngestionStatus::Completed,
                "failed" => IngestionStatus::Failed,
                "cancelled" => IngestionStatus::Cancelled,
                _ => IngestionStatus::Pending,
            };

            let stage_str: String = row.get("stage");
            let stage = match stage_str.as_str() {
                "download" => IngestionStage::Download,
                "parse" => IngestionStage::Parse,
                "chunk" => IngestionStage::Chunk,
                "embed" => IngestionStage::Embed,
                "index" => IngestionStage::Index,
                _ => IngestionStage::Download,
            };

            IngestionJob {
                id: row.get("id"),
                owner_id: row.get("owner_id"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                vector_store_id: row.get("vector_store_id"),
                file_id: row.get("file_id"),
                status,
                stage,
                error: row.get("error"),
                progress_percent: row.get("progress_percent"),
            }
        }).collect();

        Ok(Page {
            items,
            total: total as usize,
            offset: spec.offset.unwrap_or(0),
            limit: spec.limit.unwrap_or(20),
        })
    }
}
