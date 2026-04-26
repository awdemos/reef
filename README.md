# Reef

**Reef** is the Rust control plane for [CowabungaAI](https://github.com/cowabungaai/cowabungaai) — a high-performance, air-gapped AI platform with OpenAI-compatible APIs, embedded inference, and event-sourced orchestration.

> *A reef is the solid structure beneath the waves that shapes everything above it.*

---

## Why Reef?

CowabungaAI's original Python control plane worked, but suffered from architectural brittleness at scale:

- **Synchronous request-bound orchestration** blocked HTTP threads during RAG retrieval and token generation
- **Leaky storage abstractions** tightly coupled CRUD logic to Supabase-specific RLS magic
- **Insecure service mesh** used plaintext gRPC between API and inference backends
- **Deployment complexity** required managing Python virtualenvs, wheels, and CUDA variants in air-gapped environments

Reef solves these with:

| Problem | Reef Solution |
|---|---|
| Request-blocking inference | **Event-sourced RunEngine** with async worker queue |
| Supabase-only storage | **Unified StorageProtocol** — Postgres, Turso, or in-memory |
| Plaintext gRPC | **Tonic clients** with mTLS-ready transport |
| Python deployment hell | **Single static binary** + model weights |
| Hardcoded 768-dim vectors | **Backend capability registry** with dynamic model advertisement |

---

## Architecture

```
┌─────────────────────────────────────────────┐
│  HTTP Clients (UI, SDK, curl)               │
└──────────────┬──────────────────────────────┘
               │ OpenAI-compatible REST
┌──────────────▼──────────────────────────────┐
│  Reef API (axum)                            │
│  ├── Auth middleware (JWT / API key)        │
│  ├── SSE streaming (/chat/completions)      │
│  ├── Multipart file upload                  │
│  └── Request ID propagation                 │
├─────────────────────────────────────────────┤
│  Reef Engine                                │
│  ├── CapabilityRegistry (health polling)    │
│  ├── RunWorker (event-sourced state machine)│
│  ├── ModelPuller (HF Hub + NGC downloads)   │
│  ├── GrpcBackend (tonic → Python backends)  │
│  └── LlamaBackend (feature-gated native)    │
├─────────────────────────────────────────────┤
│  Reef Storage                               │
│  ├── StorageBackend trait                   │
│  ├── MemoryBackend (dev / tests)            │
│  ├── PostgresBackend (sqlx + pgvector)      │
│  └── TursoBackend (libsql / edge)           │
├─────────────────────────────────────────────┤
│  Reef Proto                                 │
│  └── Tonic generated from cowabunga_sdk     │
└─────────────────────────────────────────────┘
```

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) 1.80+
- (Optional) PostgreSQL 15+ with `pgvector`
- (Optional) A running CowabungaAI gRPC backend (vLLM, llama-cpp-python, etc.)

### Build

```bash
cd reef
cargo build --release
```

### Run (in-memory dev mode)

```bash
cargo run -- serve --dev
```

### Run with Postgres

```bash
export DATABASE_URL="postgres://user:password@localhost/cowabungaai"
cargo run -- serve --storage postgres
```

### Run with Turso

```bash
export TURSO_URL="libsql://your-db.turso.io"
export TURSO_AUTH_TOKEN="your-token"
cargo run -- serve --storage turso
```

### Test

```bash
cargo test --workspace
```

---

## CLI

```
$ reef --help

Usage: reef <COMMAND>

Commands:
  serve     Start the API server and run worker
  worker    Start only the background run worker
  pull      Pull a model from HuggingFace or NGC
  list      List cached models
  run       Run a model interactively (embedded inference)
  migrate   Run database migrations
  health    Health check against a running server
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

---

## API

Reef implements the OpenAI API specification. Key endpoints:

| Endpoint | Description |
|---|---|
| `POST /v1/chat/completions` | Chat with streaming SSE support |
| `POST /v1/embeddings` | Text embeddings |
| `POST /v1/threads` | Create conversation thread |
| `POST /v1/threads/{id}/messages` | Add message to thread |
| `POST /v1/threads/{id}/runs` | Execute assistant run |
| `POST /v1/assistants` | Create assistant |
| `POST /v1/files` | Upload file (multipart) |
| `POST /v1/vector_stores` | Create vector store |
| `POST /v1/api_keys` | Generate API key |
| `GET /metrics` | Runtime metrics |

### Example: Chat Completion

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer your-api-key" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "llama-3.1-8b",
    "messages": [{"role": "user", "content": "Hello!"}],
    "stream": true
  }'
```

### Example: Create API Key

```bash
curl http://localhost:8080/v1/api_keys \
  -H "Authorization: Bearer your-jwt" \
  -H "Content-Type: application/json" \
  -d '{"name": "production", "scopes": ["read", "write"]}'
```

---

## Model Management

### Pull from HuggingFace Hub

```bash
cargo run -- pull hf:TheBloke/Llama-2-7B-GGUF/llama-2-7b.Q4_K_M.gguf
```

### Pull from NVIDIA NGC

```bash
cargo run -- pull ngc:nvidia/nemo/megatron-gpt/1.0/model.bin
```

### List cached models

```bash
cargo run -- list
```

---

## Storage Backends

### Memory (default)

Fast, ephemeral. Perfect for development and tests.

```bash
cargo run -- serve
```

### PostgreSQL + pgvector

Production-grade with vector similarity search.

```bash
cargo run -- serve --storage postgres
```

### Turso / libSQL

Edge-deployable SQLite. Great for laptop or single-node deployments.

```bash
cargo run -- serve --storage turso
```

---

## Embedded Inference (Experimental)

Reef can run GGUF models natively via `llama.cpp` (feature-gated):

```bash
cargo build --release --features llama
cargo run -- run /path/to/model.gguf
```

> **Note:** The `llama` feature requires `llama-cpp-rs` and a C++ toolchain. See [llama-cpp-rs](https://github.com/rustformers/llama-rs) for build instructions.

---

## Configuration

Environment variables:

| Variable | Description |
|---|---|
| `DATABASE_URL` | Postgres connection string |
| `TURSO_URL` | Turso database URL |
| `TURSO_AUTH_TOKEN` | Turso authentication token |
| `HF_TOKEN` | HuggingFace Hub token (for gated models) |
| `NGC_API_KEY` | NVIDIA NGC API key |
| `RUST_LOG` | Log level (e.g., `reef=debug,tower_http=info`) |

---

## Development

### Workspace Structure

```
reef/
├── crates/
│   ├── reef/           # Binary, CLI, config
│   ├── reef-api/       # OpenAI-compatible REST API
│   ├── reef-engine/    # Run orchestration, registry, backends
│   ├── reef-storage/   # Storage trait + implementations
│   ├── reef-rag/       # Document ingestion DAG
│   └── reef-proto/     # gRPC protobuf definitions
├── migrations/         # SQL schema migrations
└── Cargo.toml          # Workspace manifest
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Specific crate
cargo test -p reef-engine --lib
cargo test -p reef-storage --lib
cargo test -p reef-api --test integration
```

---

## Roadmap

- [x] OpenAI-compatible REST API
- [x] Event-sourced run orchestration
- [x] gRPC backend integration (vLLM, llama-cpp-python)
- [x] Postgres + pgvector storage
- [x] Turso / libSQL storage
- [x] Model pulling (HF Hub, NGC)
- [x] API key management
- [x] SSE streaming
- [x] Multipart file upload
- [ ] mTLS for gRPC channels
- [ ] Distributed worker queue (Redis / NATS)
- [ ] Full Turso Tier 2 (sqlite-vec)
- [ ] Native llama.cpp inference
- [ ] OpenTelemetry tracing
- [ ] GPU scheduling (Kubernetes)

---

## License

Apache-2.0

---

*Reef is part of the CowabungaAI project — bringing AI to air-gapped environments.*
