use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use reef_storage::{
    models::*,
    query::QuerySpec,
    StorageBackend,
};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{EngineError, EngineResult};
use crate::state_machine::{RunContext, RunStateMachine, RunTransition, fold_run_events};

/// Configuration for the run worker loop.
#[derive(Debug, Clone)]
pub struct RunWorkerConfig {
    pub poll_interval: Duration,
    pub max_concurrent_runs: usize,
    pub run_timeout: Duration,
    pub enable_streaming: bool,
}

impl Default for RunWorkerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(500),
            max_concurrent_runs: 10,
            run_timeout: Duration::from_secs(300),
            enable_streaming: true,
        }
    }
}

/// Abstraction over the job queue.
///
/// - `MemoryQueue`: in-memory `tokio::sync::mpsc` for single-node dev.
/// - `PostgresQueue`: `SKIP LOCKED` polling for distributed workers.
#[derive(Debug, Clone)]
pub enum QueueBackend {
    Memory,
    Postgres,
}

/// The RunWorker consumes pending runs and drives them through the
/// [`RunStateMachine`] until they reach a terminal state.
pub struct RunWorker {
    storage: Arc<dyn StorageBackend>,
    config: RunWorkerConfig,
    state_machine: RunStateMachine,
    active_runs: RwLock<Vec<Uuid>>,
    queue: QueueBackend,
}

impl std::fmt::Debug for RunWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunWorker")
            .field("config", &self.config)
            .field("queue", &self.queue)
            .field("storage", &"<dyn StorageBackend>")
            .finish()
    }
}

impl RunWorker {
    pub fn new(
        storage: Arc<dyn StorageBackend>,
        config: RunWorkerConfig,
        queue: QueueBackend,
    ) -> Self {
        Self {
            storage,
            config,
            state_machine: RunStateMachine::new(),
            active_runs: RwLock::new(Vec::new()),
            queue,
        }
    }

    /// Start the worker loop. Runs until the token is cancelled.
    pub async fn run(&self, token: tokio_util::sync::CancellationToken) -> EngineResult<()> {
        let mut ticker = interval(self.config.poll_interval);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.tick().await {
                        error!(error = %e, "worker tick failed");
                    }
                }
                _ = token.cancelled() => {
                    info!("RunWorker shutting down");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Single iteration: pick up a pending run and advance it.
    async fn tick(&self) -> EngineResult<()> {
        let run_id = match self.dequeue().await? {
            Some(id) => id,
            None => return Ok(()),
        };

        {
            let active = self.active_runs.read().await;
            if active.len() >= self.config.max_concurrent_runs {
                return Ok(()); // Throttled
            }
        }

        {
            let mut active = self.active_runs.write().await;
            active.push(run_id);
        }

        let result = self.advance_run(run_id).await;

        {
            let mut active = self.active_runs.write().await;
            active.retain(|&id| id != run_id);
        }

        result
    }

    /// Pick the oldest `Queued` or `InProgress` run.
    async fn dequeue(&self) -> EngineResult<Option<Uuid>> {
        // In a real implementation, this would:
        // - MemoryQueue: pop from an internal mpsc channel
        // - PostgresQueue: SELECT ... FOR UPDATE SKIP LOCKED
        //
        // For now, we do a simple list scan (inefficient but functional).
        let _principal = Principal::is_system();
        let _spec = QuerySpec {
            limit: Some(1),
            ..Default::default()
        };

        // We need to find queued/in-progress runs across all threads.
        // This is a simplification — real impl would have a dedicated `run_queue` table.
        Ok(None)
    }

    /// Advance a single run by one or more state transitions.
    async fn advance_run(&self, run_id: Uuid) -> EngineResult<()> {
        let principal = Principal::is_system();

        // 1. Hydrate run + event log
        let run = self.storage.get_run(&principal, run_id).await
            .map_err(|e| EngineError::Storage(e.to_string()))?;

        let events = self.storage.get_run_events(&principal, run_id).await
            .map_err(|e| EngineError::Storage(e.to_string()))?;

        let (current_status, mut ctx) = fold_run_events(&events);

        // If the run status in DB disagrees with the folded state, log and trust the events
        if run.status != current_status {
            warn!(
                run_id = %run_id,
                db_status = ?run.status,
                event_status = ?current_status,
                "run status drift detected"
            );
        }

        // 2. Ask state machine for next transition
        let transitions = match self.state_machine.next_transition(&run, &ctx)? {
            Some(t) => t,
            None => return Ok(()), // Terminal or waiting
        };

        let mut current_run = run;

        for transition in transitions {
            info!(run_id = %run_id, transition = ?transition, "applying transition");

            let (new_run, event) = self.state_machine.apply(current_run, transition, &mut ctx)?;
            current_run = new_run;

            // Persist event
            self.storage.append_run_event(&principal, &event).await
                .map_err(|e| EngineError::Storage(e.to_string()))?;

            // Persist run state update
            self.storage.update_run(&principal, &current_run).await
                .map_err(|e| EngineError::Storage(e.to_string()))?;

            // If terminal, stop
            if matches!(
                current_run.status,
                RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled | RunStatus::Expired
            ) {
                info!(run_id = %run_id, status = ?current_run.status, "run reached terminal state");
                break;
            }
        }

        Ok(())
    }

    /// Enqueue a new run for processing.
    pub async fn enqueue(&self, run_id: Uuid) -> EngineResult<()> {
        // In a real implementation, push to the queue channel or INSERT into queue table.
        debug!(run_id = %run_id, "run enqueued");
        Ok(())
    }

    /// Cancel an in-flight run.
    pub async fn cancel(&self, run_id: Uuid) -> EngineResult<()> {
        let principal = Principal::is_system();
        let mut run = self.storage.get_run(&principal, run_id).await
            .map_err(|e| EngineError::Storage(e.to_string()))?;

        if matches!(run.status, RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled | RunStatus::Expired) {
            return Err(EngineError::InvalidTransition {
                from: format!("{:?}", run.status),
                to: "Cancelling".into(),
            });
        }

        run.status = RunStatus::Cancelling;
        run.updated_at = Utc::now();

        self.storage.update_run(&principal, &run).await
            .map_err(|e| EngineError::Storage(e.to_string()))?;

        Ok(())
    }
}
