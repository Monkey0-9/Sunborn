use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Vector Clock for multi-region active-active causal ordering and anti-entropy synchronization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    pub clocks: HashMap<String, u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalityRelation {
    Equal,
    Dominates,
    DominatedBy,
    Concurrent,
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

impl VectorClock {
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    pub fn increment(&mut self, node_id: &str) -> u64 {
        let entry = self.clocks.entry(node_id.to_string()).or_insert(0);
        *entry += 1;
        *entry
    }

    pub fn get(&self, node_id: &str) -> u64 {
        self.clocks.get(node_id).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &val) in &other.clocks {
            let entry = self.clocks.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(val);
        }
    }

    pub fn compare(&self, other: &VectorClock) -> CausalityRelation {
        let mut greater = false;
        let mut lesser = false;

        let all_nodes: HashSet<_> = self.clocks.keys().chain(other.clocks.keys()).collect();
        for node in all_nodes {
            let a = self.get(node);
            let b = other.get(node);
            if a > b {
                greater = true;
            } else if a < b {
                lesser = true;
            }
        }

        match (greater, lesser) {
            (false, false) => CausalityRelation::Equal,
            (true, false) => CausalityRelation::Dominates,
            (false, true) => CausalityRelation::DominatedBy,
            (true, true) => CausalityRelation::Concurrent,
        }
    }
}

/// Anti-entropy synchronization digest across distributed points-of-presence (PoPs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSyncDigest {
    pub node_id: String,
    pub region: String,
    pub vector_clock: VectorClock,
    pub state_merkle_root: String,
    pub active_leases: usize,
    pub timestamp_utc: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_causality_and_merge() {
        let mut v1 = VectorClock::new();
        let mut v2 = VectorClock::new();

        v1.increment("node-us-east-1");
        assert_eq!(v1.compare(&v2), CausalityRelation::Dominates);
        assert_eq!(v2.compare(&v1), CausalityRelation::DominatedBy);

        v2.increment("node-eu-west-1");
        assert_eq!(v1.compare(&v2), CausalityRelation::Concurrent);

        v1.merge(&v2);
        assert_eq!(v1.get("node-us-east-1"), 1);
        assert_eq!(v1.get("node-eu-west-1"), 1);
        assert_eq!(v1.compare(&v2), CausalityRelation::Dominates);
    }
}
