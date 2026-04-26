-- Reef initial schema for PostgreSQL + pgvector

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgvector";

-- Threads
CREATE TABLE IF NOT EXISTS threads (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    title TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_threads_owner ON threads(owner_id);
CREATE INDEX idx_threads_created ON threads(created_at DESC);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    thread_id UUID NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    run_id UUID,
    role TEXT NOT NULL,
    content JSONB NOT NULL DEFAULT '[]',
    annotations JSONB NOT NULL DEFAULT '[]',
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_messages_thread ON messages(thread_id);
CREATE INDEX idx_messages_created ON messages(created_at ASC);

-- Runs
CREATE TABLE IF NOT EXISTS runs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    thread_id UUID NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    assistant_id UUID,
    status TEXT NOT NULL DEFAULT 'queued',
    model TEXT NOT NULL,
    instructions TEXT,
    tools JSONB NOT NULL DEFAULT '[]',
    temperature REAL,
    max_prompt_tokens INT,
    max_completion_tokens INT,
    metadata JSONB NOT NULL DEFAULT '{}',
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    expired_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,
    last_error JSONB
);

CREATE INDEX idx_runs_thread ON runs(thread_id);
CREATE INDEX idx_runs_status ON runs(status);
CREATE INDEX idx_runs_created ON runs(created_at DESC);

-- Run events (append-only event log)
CREATE TABLE IF NOT EXISTS run_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    run_id UUID NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_run_events_run ON run_events(run_id);
CREATE INDEX idx_run_events_created ON run_events(created_at ASC);

-- Assistants
CREATE TABLE IF NOT EXISTS assistants (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    name TEXT,
    description TEXT,
    model TEXT NOT NULL,
    instructions TEXT,
    tools JSONB NOT NULL DEFAULT '[]',
    tool_resources JSONB NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    temperature REAL,
    top_p REAL,
    response_format JSONB
);

CREATE INDEX idx_assistants_owner ON assistants(owner_id);

-- Vector stores
CREATE TABLE IF NOT EXISTS vector_stores (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    name TEXT,
    bytes BIGINT NOT NULL DEFAULT 0,
    file_counts JSONB NOT NULL DEFAULT '{"in_progress":0,"completed":0,"failed":0,"cancelled":0,"total":0}',
    status TEXT NOT NULL DEFAULT 'in_progress',
    expires_after JSONB,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_vector_stores_owner ON vector_stores(owner_id);

-- Vector content (with pgvector)
CREATE TABLE IF NOT EXISTS vector_content (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    vector_store_id UUID NOT NULL REFERENCES vector_stores(id) ON DELETE CASCADE,
    file_id UUID NOT NULL,
    embedding_model TEXT NOT NULL,
    dimension INT NOT NULL,
    embedding VECTOR(768),
    content TEXT NOT NULL,
    content_type TEXT NOT NULL DEFAULT 'text',
    metadata JSONB NOT NULL DEFAULT '{}',
    idx INT NOT NULL
);

CREATE INDEX idx_vector_content_store ON vector_content(vector_store_id);
CREATE INDEX idx_vector_content_file ON vector_content(file_id);

-- Match function for vector similarity search
CREATE OR REPLACE FUNCTION match_vectors(
    query_embedding VECTOR(768),
    match_vector_store_id UUID,
    match_count INT DEFAULT 5
)
RETURNS TABLE(
    id UUID,
    vector_store_id UUID,
    file_id UUID,
    content TEXT,
    content_type TEXT,
    metadata JSONB,
    idx INT,
    similarity FLOAT
)
LANGUAGE SQL STABLE
AS $$
    SELECT
        vector_content.id,
        vector_content.vector_store_id,
        vector_content.file_id,
        vector_content.content,
        vector_content.content_type,
        vector_content.metadata,
        vector_content.idx,
        1 - (vector_content.embedding <=> query_embedding) AS similarity
    FROM vector_content
    WHERE vector_content.vector_store_id = match_vector_store_id
    ORDER BY vector_content.embedding <=> query_embedding
    LIMIT match_count;
$$;

-- Files
CREATE TABLE IF NOT EXISTS file_objects (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    bytes BIGINT NOT NULL DEFAULT 0,
    filename TEXT NOT NULL,
    purpose TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'uploaded',
    status_details TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'
);

CREATE INDEX idx_files_owner ON file_objects(owner_id);

-- API keys
CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    key_hash TEXT NOT NULL UNIQUE,
    key_preview TEXT NOT NULL,
    name TEXT NOT NULL,
    scopes JSONB NOT NULL DEFAULT '[]',
    expires_at TIMESTAMPTZ,
    last_used_at TIMESTAMPTZ
);

CREATE INDEX idx_api_keys_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_owner ON api_keys(owner_id);

-- Ingestion jobs
CREATE TABLE IF NOT EXISTS ingestion_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    vector_store_id UUID NOT NULL REFERENCES vector_stores(id) ON DELETE CASCADE,
    file_id UUID NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    stage TEXT NOT NULL DEFAULT 'download',
    error TEXT,
    progress_percent INT NOT NULL DEFAULT 0
);

CREATE INDEX idx_ingestion_jobs_store ON ingestion_jobs(vector_store_id);
