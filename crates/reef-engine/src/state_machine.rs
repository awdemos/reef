use chrono::Utc;
use reef_storage::{
    models::*,
};
use std::collections::HashMap;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{EngineError, EngineResult};

/// The deterministic state machine for Run lifecycle management.
///
/// This struct is pure logic — it does not perform I/O.  The [`RunWorker`]
/// drives I/O and calls `transition()` to compute the next state.
#[derive(Debug, Clone)]
pub struct RunStateMachine;

/// Context accumulated while a run is executing.
/// Fed into the state machine to determine valid transitions.
#[derive(Debug, Clone, Default)]
pub struct RunContext {
    pub retrieved_chunks: Vec<ScoredChunkContext>,
    pub token_count: usize,
    pub tool_outputs: HashMap<String, String>,
    pub stream_index: usize,
    pub error: Option<RunError>,
}

#[derive(Debug, Clone)]
pub struct ScoredChunkContext {
    pub file_id: Uuid,
    pub content: String,
    pub score: f32,
    pub index: usize,
}

/// Valid transitions emitted by the state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum RunTransition {
    Start,
    Retrieve { query: String },
    AssembleContext,
    Generate,
    StreamToken { delta: String },
    CreateToolCall { tool_call_id: String, tool_type: String, arguments: String },
    CompleteToolCall { tool_call_id: String, output: String },
    Complete,
    Fail { error: RunError },
    Cancel,
    Expire,
}

impl RunStateMachine {
    pub fn new() -> Self {
        Self
    }

    /// Given current run state and context, compute the next transition(s).
    /// Returns `None` when the run has reached a terminal state.
    pub fn next_transition(
        &self,
        run: &Run,
        ctx: &RunContext,
    ) -> EngineResult<Option<Vec<RunTransition>>> {
        use RunStatus::*;
        use RunTransition::*;

        match run.status {
            Queued => Ok(Some(vec![Start, Retrieve {
                query: Self::build_retrieval_query(run),
            }])),

            InProgress => {
                // If we have an error, fail immediately
                if let Some(ref err) = ctx.error {
                    return Ok(Some(vec![Fail { error: err.clone() }]));
                }

                // If we haven't retrieved yet
                if run.tools.iter().any(|t| t.r#type == "file_search") && ctx.retrieved_chunks.is_empty() {
                    return Ok(Some(vec![Retrieve {
                        query: Self::build_retrieval_query(run),
                    }]));
                }

                // If retrieval is done but context not assembled
                if ctx.token_count == 0 && !ctx.retrieved_chunks.is_empty() {
                    return Ok(Some(vec![AssembleContext]));
                }

                // If generation hasn't started
                if run.started_at.is_some() && ctx.token_count > 0 && ctx.stream_index == 0 {
                    return Ok(Some(vec![Generate]));
                }

                // Streaming in progress — in a real impl, this is driven by
                // backend tokens, not the state machine. The state machine
                // just validates that `StreamToken` is legal while InProgress.
                Ok(None) // Tokens are pushed externally
            }

            RequiresAction => {
                // Check if all tool calls have outputs
                let pending: Vec<String> = run
                    .tools
                    .iter()
                    .filter(|t| t.r#type != "file_search")
                    .map(|t| format!("tool_{}", t.r#type)) // simplified
                    .filter(|id| !ctx.tool_outputs.contains_key(id))
                    .collect();

                if pending.is_empty() {
                    Ok(Some(vec![Generate]))
                } else {
                    Ok(None) // Waiting for tool outputs
                }
            }

            Cancelling => Ok(Some(vec![Cancel])),

            // Terminal states
            Completed | Failed | Cancelled | Expired => Ok(None),
        }
    }

    /// Apply a transition to a Run, producing the mutated Run and the RunEvent
    /// that should be appended to the event log.
    pub fn apply(
        &self,
        mut run: Run,
        transition: RunTransition,
        ctx: &mut RunContext,
    ) -> EngineResult<(Run, RunEvent)> {
        use RunStatus::*;
        use RunTransition::*;

        let event = match transition {
            Start => {
                run.status = InProgress;
                run.started_at = Some(Utc::now());
                RunEventType::RunCreated
            }

            Retrieve { query } => {
                RunEventType::RetrievalStarted { query }
            }

            AssembleContext => {
                RunEventType::ContextAssembled { token_count: ctx.token_count }
            }

            Generate => {
                RunEventType::GenerationStarted
            }

            StreamToken { delta } => {
                let idx = ctx.stream_index;
                ctx.stream_index += 1;
                RunEventType::TokenStreamed { delta, index: idx }
            }

            CreateToolCall { tool_call_id, tool_type, arguments } => {
                RunEventType::ToolCallCreated { tool_call_id, tool_type, arguments }
            }

            CompleteToolCall { tool_call_id, output } => {
                ctx.tool_outputs.insert(tool_call_id.clone(), output.clone());
                RunEventType::ToolCallCompleted { tool_call_id, output }
            }

            Complete => {
                run.status = Completed;
                run.completed_at = Some(Utc::now());
                RunEventType::RunCompleted
            }

            Fail { error } => {
                run.status = Failed;
                run.failed_at = Some(Utc::now());
                run.last_error = Some(error.clone());
                RunEventType::RunFailed { error }
            }

            Cancel => {
                run.status = Cancelled;
                run.completed_at = Some(Utc::now());
                RunEventType::RunCancelled
            }

            Expire => {
                run.status = Expired;
                run.expired_at = Some(Utc::now());
                RunEventType::RunFailed {
                    error: RunError {
                        code: "expired".into(),
                        message: "Run exceeded maximum execution time".into(),
                    }
                }
            }
        };

        run.updated_at = Utc::now();

        let run_event = RunEvent {
            id: Uuid::new_v4(),
            run_id: run.id,
            created_at: Utc::now(),
            event_type: event,
        };

        Ok((run, run_event))
    }

    /// Is the given transition valid from the current state?
    pub fn is_valid_transition(&self, from: &RunStatus, transition: &RunTransition) -> bool {
        use RunStatus::*;
        use RunTransition::*;

        match (from, transition) {
            (Queued, Start | Retrieve { .. }) => true,
            (InProgress, Retrieve { .. } | AssembleContext | Generate | StreamToken { .. } | CreateToolCall { .. } | CompleteToolCall { .. } | Complete | Fail { .. } | Expire) => true,
            (RequiresAction, CompleteToolCall { .. } | Generate | Fail { .. }) => true,
            (Cancelling, Cancel | Fail { .. }) => true,
            _ => false,
        }
    }

    fn build_retrieval_query(run: &Run) -> String {
        // In a real implementation, this would inspect the thread messages
        // to build the retrieval query. For now, use the run instructions or a placeholder.
        run.instructions.clone().unwrap_or_else(|| "user query".into())
    }
}

/// Reconstruct a Run's current state by folding over its event log.
pub fn fold_run_events(events: &[RunEvent]) -> (RunStatus, RunContext) {
    let mut status = RunStatus::Queued;
    let mut ctx = RunContext::default();

    for event in events {
        match &event.event_type {
            RunEventType::RunCreated => status = RunStatus::InProgress,
            RunEventType::RetrievalCompleted { .. } => {
                // In a real impl, we'd hydrate chunk metadata here
            }
            RunEventType::ContextAssembled { token_count } => {
                ctx.token_count = *token_count;
            }
            RunEventType::TokenStreamed { index, .. } => {
                ctx.stream_index = *index + 1;
            }
            RunEventType::ToolCallCompleted { tool_call_id, output } => {
                ctx.tool_outputs.insert(tool_call_id.clone(), output.clone());
            }
            RunEventType::RunCompleted => status = RunStatus::Completed,
            RunEventType::RunFailed { .. } => status = RunStatus::Failed,
            RunEventType::RunCancelled => status = RunStatus::Cancelled,
            _ => {}
        }
    }

    (status, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_run(status: RunStatus) -> Run {
        Run {
            id: Uuid::new_v4(),
            owner_id: "test".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            thread_id: Uuid::new_v4(),
            assistant_id: None,
            status,
            model: "test-model".into(),
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
        }
    }

    #[test]
    fn test_queued_to_in_progress() {
        let sm = RunStateMachine::new();
        let run = make_run(RunStatus::Queued);
        let ctx = RunContext::default();

        let transitions = sm.next_transition(&run, &ctx).unwrap().unwrap();
        assert_eq!(transitions.len(), 2);
        assert!(matches!(transitions[0], RunTransition::Start));
        assert!(matches!(transitions[1], RunTransition::Retrieve { .. }));
    }

    #[test]
    fn test_apply_start_transition() {
        let sm = RunStateMachine::new();
        let run = make_run(RunStatus::Queued);
        let mut ctx = RunContext::default();

        let (new_run, event) = sm.apply(run, RunTransition::Start, &mut ctx).unwrap();
        assert_eq!(new_run.status, RunStatus::InProgress);
        assert!(new_run.started_at.is_some());
        assert!(matches!(event.event_type, RunEventType::RunCreated));
    }

    #[test]
    fn test_apply_complete_transition() {
        let sm = RunStateMachine::new();
        let run = make_run(RunStatus::InProgress);
        let mut ctx = RunContext::default();

        let (new_run, event) = sm.apply(run, RunTransition::Complete, &mut ctx).unwrap();
        assert_eq!(new_run.status, RunStatus::Completed);
        assert!(new_run.completed_at.is_some());
        assert!(matches!(event.event_type, RunEventType::RunCompleted));
    }

    #[test]
    fn test_apply_fail_transition() {
        let sm = RunStateMachine::new();
        let run = make_run(RunStatus::InProgress);
        let mut ctx = RunContext::default();

        let error = RunError {
            code: "test_error".into(),
            message: "something went wrong".into(),
        };

        let (new_run, event) = sm.apply(run, RunTransition::Fail { error: error.clone() }, &mut ctx).unwrap();
        assert_eq!(new_run.status, RunStatus::Failed);
        assert_eq!(new_run.last_error, Some(error));
        assert!(matches!(event.event_type, RunEventType::RunFailed { .. }));
    }

    #[test]
    fn test_invalid_transition() {
        let sm = RunStateMachine::new();
        assert!(!sm.is_valid_transition(&RunStatus::Completed, &RunTransition::Start));
        assert!(!sm.is_valid_transition(&RunStatus::Queued, &RunTransition::Complete));
        assert!(sm.is_valid_transition(&RunStatus::InProgress, &RunTransition::Complete));
    }

    #[test]
    fn test_fold_run_events() {
        let run_id = Uuid::new_v4();
        let events = vec![
            RunEvent {
                id: Uuid::new_v4(),
                run_id,
                created_at: Utc::now(),
                event_type: RunEventType::RunCreated,
            },
            RunEvent {
                id: Uuid::new_v4(),
                run_id,
                created_at: Utc::now(),
                event_type: RunEventType::ContextAssembled { token_count: 42 },
            },
            RunEvent {
                id: Uuid::new_v4(),
                run_id,
                created_at: Utc::now(),
                event_type: RunEventType::RunCompleted,
            },
        ];

        let (status, ctx) = fold_run_events(&events);
        assert_eq!(status, RunStatus::Completed);
        assert_eq!(ctx.token_count, 42);
    }

    #[test]
    fn test_terminal_states_return_none() {
        let sm = RunStateMachine::new();
        for status in [RunStatus::Completed, RunStatus::Failed, RunStatus::Cancelled, RunStatus::Expired] {
            let run = make_run(status);
            let ctx = RunContext::default();
            assert!(sm.next_transition(&run, &ctx).unwrap().is_none());
        }
    }
}
