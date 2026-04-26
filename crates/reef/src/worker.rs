use std::sync::Arc;

use reef_engine::{RunWorker, RunWorkerConfig, QueueBackend};
use reef_storage::MemoryBackend;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub async fn start(storage: String) -> anyhow::Result<()> {
    let storage: Arc<dyn reef_storage::StorageBackend> = match storage.as_str() {
        "memory" => {
            info!("using in-memory storage backend");
            Arc::new(MemoryBackend::new())
        }
        other => anyhow::bail!("unknown storage backend: {}", other),
    };

    let worker = RunWorker::new(
        storage,
        RunWorkerConfig::default(),
        QueueBackend::Memory,
    );

    let token = CancellationToken::new();
    let token_clone = token.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        token_clone.cancel();
    });

    worker.run(token).await?;
    Ok(())
}
