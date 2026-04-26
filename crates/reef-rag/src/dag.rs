use std::collections::HashMap;

use reef_storage::models::{IngestionJob, IngestionStage, IngestionStatus, VectorChunk};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{RagError, RagResult};

/// A node in the ingestion DAG.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagNode {
    pub id: Uuid,
    pub stage: IngestionStage,
    pub inputs: Vec<Uuid>,
    pub outputs: Vec<NodeOutput>,
    pub status: NodeStatus,
    pub error: Option<String>,
    pub retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeOutput {
    RawBytes(Vec<u8>),
    ParsedDocument(crate::parser::ParsedDocument),
    Chunks(Vec<String>),
    Embeddings(Vec<Vec<f32>>),
    Indexed(Vec<VectorChunk>),
}

/// A directed edge representing data flow between stages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DagEdge {
    pub from: Uuid,
    pub to: Uuid,
}

/// The ingestion DAG orchestrates file → parse → chunk → embed → index.
///
/// Each file ingestion creates one DAG. Stages are executed sequentially
/// but can be retried independently on failure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestionDag {
    pub job_id: Uuid,
    pub vector_store_id: Uuid,
    pub file_id: Uuid,
    pub nodes: HashMap<Uuid, DagNode>,
    pub edges: Vec<DagEdge>,
}

impl IngestionDag {
    pub fn new(job_id: Uuid, vector_store_id: Uuid, file_id: Uuid) -> Self {
        let mut nodes = HashMap::new();
        let mut edges = Vec::new();

        let download_id = Uuid::new_v4();
        let parse_id = Uuid::new_v4();
        let chunk_id = Uuid::new_v4();
        let embed_id = Uuid::new_v4();
        let index_id = Uuid::new_v4();

        nodes.insert(download_id, DagNode {
            id: download_id,
            stage: IngestionStage::Download,
            inputs: vec![],
            outputs: vec![],
            status: NodeStatus::Pending,
            error: None,
            retries: 0,
        });

        nodes.insert(parse_id, DagNode {
            id: parse_id,
            stage: IngestionStage::Parse,
            inputs: vec![download_id],
            outputs: vec![],
            status: NodeStatus::Pending,
            error: None,
            retries: 0,
        });

        nodes.insert(chunk_id, DagNode {
            id: chunk_id,
            stage: IngestionStage::Chunk,
            inputs: vec![parse_id],
            outputs: vec![],
            status: NodeStatus::Pending,
            error: None,
            retries: 0,
        });

        nodes.insert(embed_id, DagNode {
            id: embed_id,
            stage: IngestionStage::Embed,
            inputs: vec![chunk_id],
            outputs: vec![],
            status: NodeStatus::Pending,
            error: None,
            retries: 0,
        });

        nodes.insert(index_id, DagNode {
            id: index_id,
            stage: IngestionStage::Index,
            inputs: vec![embed_id],
            outputs: vec![],
            status: NodeStatus::Pending,
            error: None,
            retries: 0,
        });

        edges.push(DagEdge { from: download_id, to: parse_id });
        edges.push(DagEdge { from: parse_id, to: chunk_id });
        edges.push(DagEdge { from: chunk_id, to: embed_id });
        edges.push(DagEdge { from: embed_id, to: index_id });

        Self {
            job_id,
            vector_store_id,
            file_id,
            nodes,
            edges,
        }
    }

    /// Find the next node(s) that are ready to execute (all inputs completed).
    pub fn ready_nodes(&self) -> Vec<&DagNode> {
        self.nodes
            .values()
            .filter(|n| n.status == NodeStatus::Pending)
            .filter(|n| {
                n.inputs.iter().all(|input_id| {
                    self.nodes
                        .get(input_id)
                        .map(|input| input.status == NodeStatus::Completed)
                        .unwrap_or(true)
                })
            })
            .collect()
    }

    /// Overall DAG status derived from node statuses.
    pub fn overall_status(&self) -> IngestionStatus {
        let all_completed = self.nodes.values().all(|n| n.status == NodeStatus::Completed);
        let any_failed = self.nodes.values().any(|n| n.status == NodeStatus::Failed);
        let any_running = self.nodes.values().any(|n| n.status == NodeStatus::Running);

        if all_completed {
            IngestionStatus::Completed
        } else if any_failed {
            IngestionStatus::Failed
        } else if any_running {
            IngestionStatus::Running
        } else {
            IngestionStatus::Pending
        }
    }

    /// Completion percentage (0-100).
    pub fn progress_percent(&self) -> i32 {
        let total = self.nodes.len() as i32;
        if total == 0 {
            return 0;
        }
        let completed = self.nodes.values().filter(|n| n.status == NodeStatus::Completed).count() as i32;
        (completed * 100) / total
    }
}
