pub mod api_keys;
pub mod assistants;
pub mod chat;
pub mod dev;
pub mod files;
pub mod metrics;
pub mod threads;
pub mod vector_stores;

use std::sync::Arc;

use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::state::AppState;

/// Build the OpenAI-compatible REST router.
pub fn build_router(state: Arc<AppState>) -> Router {
    let mut router = Router::new()
        // Health
        .route("/health", get(health_handler))
        // Threads
        .route("/v1/threads", post(threads::create_thread).get(threads::list_threads))
        .route(
            "/v1/threads/{thread_id}",
            get(threads::get_thread).post(threads::update_thread).delete(threads::delete_thread),
        )
        .route(
            "/v1/threads/{thread_id}/messages",
            post(threads::create_message).get(threads::list_messages),
        )
        .route(
            "/v1/threads/{thread_id}/runs",
            post(threads::create_run).get(threads::list_runs),
        )
        .route("/v1/threads/{thread_id}/runs/{run_id}", get(threads::get_run))
        .route(
            "/v1/threads/{thread_id}/runs/{run_id}/cancel",
            post(threads::cancel_run),
        )
        // Assistants
        .route(
            "/v1/assistants",
            post(assistants::create_assistant).get(assistants::list_assistants),
        )
        .route(
            "/v1/assistants/{assistant_id}",
            get(assistants::get_assistant)
                .post(assistants::update_assistant)
                .delete(assistants::delete_assistant),
        )
        // Chat completions
        .route("/v1/chat/completions", post(chat::chat_completions))
        // Embeddings
        .route("/v1/embeddings", post(chat::create_embeddings))
        // Vector stores
        .route(
            "/v1/vector_stores",
            post(vector_stores::create_vector_store).get(vector_stores::list_vector_stores),
        )
        .route(
            "/v1/vector_stores/{vector_store_id}",
            get(vector_stores::get_vector_store).delete(vector_stores::delete_vector_store),
        )
        // Files
        .route("/v1/files", post(files::upload_file).get(files::list_files))
        .route("/v1/files/{file_id}", get(files::get_file).delete(files::delete_file))
        // API Keys
        .route("/v1/api_keys", post(api_keys::create_api_key).get(api_keys::list_api_keys))
        .route("/v1/api_keys/{key_id}", delete(api_keys::delete_api_key))
        // Metrics
        .route("/metrics", get(metrics::metrics));

    if state.config.enable_dev_routes {
        router = router.route("/dev/models", get(dev::list_models));
    }

    router.with_state(state)
}

async fn health_handler() -> &'static str {
    "ok"
}
