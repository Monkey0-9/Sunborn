use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Computed reuse execution plan distinguishing reusable nodes from those requiring recomputation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubgraphReusePlan {
    pub reusable_nodes: HashSet<String>,
    pub recompute_nodes: HashSet<String>,
    pub prohibited_nodes: HashSet<String>,
}

impl SubgraphReusePlan {
    pub fn is_reusable(&self, node_id: &str) -> bool {
        self.reusable_nodes.contains(node_id)
    }

    pub fn reusable_list(&self) -> Vec<String> {
        let mut list: Vec<String> = self.reusable_nodes.iter().cloned().collect();
        list.sort();
        list
    }

    pub fn recompute_list(&self) -> Vec<String> {
        let mut list: Vec<String> = self.recompute_nodes.iter().cloned().collect();
        list.sort();
        list
    }
}
