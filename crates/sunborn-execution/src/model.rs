use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub version: String,
    pub is_deterministic: bool,
    pub is_side_effect_free: bool,
    pub default_ttl_secs: Option<u64>,
}

impl ToolDefinition {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        deterministic: bool,
        side_effect_free: bool,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            is_deterministic: deterministic,
            is_side_effect_free: side_effect_free,
            default_ttl_secs: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool_name: String,
    pub tool_version: String,
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub context_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub signature: String,
    pub output: serde_json::Value,
    pub is_error: bool,
    pub execution_latency_ms: u64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub tool_call: Option<ToolCall>,
    pub is_deterministic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "details")]
pub enum NodeExecutionStatus {
    Cached {
        signature: String,
        output: serde_json::Value,
        latency_saved_ms: u64,
    },
    PendingExecution,
    Executed {
        signature: String,
        output: serde_json::Value,
    },
    Failed {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionPlan {
    pub nodes: Vec<ExecutionNode>,
}

impl ExecutionPlan {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    pub fn add_node(&mut self, node: ExecutionNode) {
        self.nodes.push(node);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEvaluation {
    pub total_nodes: usize,
    pub cached_nodes: usize,
    pub reused_node_ids: Vec<String>,
    pub pending_node_ids: Vec<String>,
    pub estimated_latency_saved_ms: u64,
    pub node_statuses: HashMap<String, NodeExecutionStatus>,
}
