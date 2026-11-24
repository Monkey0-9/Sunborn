use crate::effect_class::EffectClass;
use crate::subgraph_reuse::SubgraphReusePlan;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Node in an agent execution Directed Acyclic Graph (DAG).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DAGNode {
    pub id: String,
    pub effect: EffectClass,
    pub dependencies: Vec<String>,
    pub input_digest: Option<String>,
}

impl DAGNode {
    pub fn new(id: impl Into<String>, effect: EffectClass) -> Self {
        Self {
            id: id.into(),
            effect,
            dependencies: Vec::new(),
            input_digest: None,
        }
    }

    pub fn with_dep(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    pub fn with_input_digest(mut self, digest: impl Into<String>) -> Self {
        self.input_digest = Some(digest.into());
        self
    }
}

/// Agent execution workflow graph.
#[derive(Debug, Clone, Default)]
pub struct AgentExecutionDAG {
    nodes: HashMap<String, DAGNode>,
}

impl AgentExecutionDAG {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, node: DAGNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    pub fn get_node(&self, id: &str) -> Option<&DAGNode> {
        self.nodes.get(id)
    }

    pub fn all_nodes(&self) -> &HashMap<String, DAGNode> {
        &self.nodes
    }

    /// Computes all transitive ancestors of a given node in the DAG.
    pub fn get_ancestors(&self, start_id: &str) -> HashSet<String> {
        let mut ancestors = HashSet::new();
        let mut queue = Vec::new();

        if let Some(node) = self.nodes.get(start_id) {
            for dep in &node.dependencies {
                queue.push(dep.clone());
            }
        }

        while let Some(current) = queue.pop() {
            if ancestors.insert(current.clone()) {
                if let Some(parent) = self.nodes.get(&current) {
                    for dep in &parent.dependencies {
                        queue.push(dep.clone());
                    }
                }
            }
        }

        ancestors
    }
}

/// Planner that analyzes agent DAGs and identifies safe subgraphs for partial execution reuse.
pub struct SubgraphPlanner;

impl Default for SubgraphPlanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SubgraphPlanner {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates each node in the DAG.
    /// A node is reusable iff:
    /// 1. Its own effect is inherently reusable (Pure, ReadOnlyStateful, ExternalMutableRead).
    /// 2. NONE of its transitive ancestors are mutating (NonIdempotentMutation) or unknown (UnknownAbstain).
    pub fn analyze(&self, dag: &AgentExecutionDAG) -> SubgraphReusePlan {
        let mut reusable_nodes = HashSet::new();
        let mut recompute_nodes = HashSet::new();
        let mut prohibited_nodes = HashSet::new();

        for (id, node) in dag.all_nodes() {
            // Check own effect class
            if !node.effect.is_inherently_reusable() {
                prohibited_nodes.insert(id.clone());
                recompute_nodes.insert(id.clone());
                continue;
            }

            // Check transitive ancestors for mutation taint
            let ancestors = dag.get_ancestors(id);
            let mut tainted = false;

            for anc_id in ancestors {
                if let Some(anc_node) = dag.get_node(&anc_id) {
                    if !anc_node.effect.is_inherently_reusable() {
                        tainted = true;
                        break;
                    }
                }
            }

            if tainted {
                recompute_nodes.insert(id.clone());
            } else {
                reusable_nodes.insert(id.clone());
            }
        }

        SubgraphReusePlan {
            reusable_nodes,
            recompute_nodes,
            prohibited_nodes,
        }
    }
}
