// ============================================================================
// crates/sunborn-cache/src/gdsf.rs
// Genuine Cost-Aware Greedy-Dual-Size-Frequency (GDSF) Byte-Bounded Cache
// Unifies sunborn-core FinOps cost avoidance with byte-accurate eviction.
// ============================================================================
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

#[derive(Debug, Clone)]
pub struct CachedObject<V> {
    pub value: V,
    pub size_bytes: usize,
    pub origin_cost_micro_usd: u64,
    pub origin_latency_ms: u64,
    pub frequency: u64,
    pub priority_key_bits: u64, // IEEE-754 f64 bits for deterministic total ordering
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct PriorityEntry {
    priority_bits: u64,
    key: String,
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority_bits
            .cmp(&other.priority_bits)
            .then_with(|| self.key.cmp(&other.key))
    }
}

impl PartialOrd for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub struct FinOpsGdsfCache<V> {
    max_bytes: usize,
    current_bytes: usize,
    inflation_clock: f64,
    latency_weight_gamma: f64,
    entries: HashMap<String, CachedObject<V>>,
    priority_tree: BTreeSet<PriorityEntry>,
}

impl<V: Clone> FinOpsGdsfCache<V> {
    pub fn new(max_bytes: usize, latency_weight_gamma: f64) -> Self {
        Self {
            max_bytes: max_bytes.max(1024),
            current_bytes: 0,
            inflation_clock: 0.0,
            latency_weight_gamma,
            entries: HashMap::new(),
            priority_tree: BTreeSet::new(),
        }
    }

    #[inline]
    fn compute_priority(
        &self,
        freq: u64,
        cost_micro_usd: u64,
        latency_ms: u64,
        size_bytes: usize,
    ) -> u64 {
        let size = (size_bytes.max(1)) as f64;
        let economic_utility =
            (cost_micro_usd as f64) + self.latency_weight_gamma * (latency_ms as f64);
        let score = self.inflation_clock + (freq as f64) * (economic_utility.max(1.0) / size);
        score.to_bits()
    }

    pub fn get(&mut self, key: &str, now_ms: u64) -> Option<V> {
        let obj = self.entries.get_mut(key)?;
        if now_ms >= obj.expires_at_ms {
            self.remove(key);
            return None;
        }

        // Remove old priority node, increment frequency, and re-insert with updated GDSF score
        self.priority_tree.remove(&PriorityEntry {
            priority_bits: obj.priority_key_bits,
            key: key.to_string(),
        });
        obj.frequency = obj.frequency.saturating_add(1);
        let new_freq = obj.frequency;
        let cost = obj.origin_cost_micro_usd;
        let lat = obj.origin_latency_ms;
        let size = obj.size_bytes;
        let val = obj.value.clone();

        let new_priority = self.compute_priority(new_freq, cost, lat, size);
        if let Some(entry) = self.entries.get_mut(key) {
            entry.priority_key_bits = new_priority;
        }
        self.priority_tree.insert(PriorityEntry {
            priority_bits: new_priority,
            key: key.to_string(),
        });
        Some(val)
    }

    pub fn insert(
        &mut self,
        key: String,
        value: V,
        size_bytes: usize,
        origin_cost_micro_usd: u64,
        origin_latency_ms: u64,
        expires_at_ms: u64,
    ) -> bool {
        if size_bytes > self.max_bytes {
            return false; // Single item exceeds entire cache byte budget; reject admission
        }
        self.remove(&key);

        // Evict lowest FinOps-utility-per-byte entries until sufficient byte headroom exists
        while self.current_bytes + size_bytes > self.max_bytes {
            let Some(lowest) = self.priority_tree.pop_first() else {
                break;
            };
            if let Some(evicted) = self.entries.remove(&lowest.key) {
                self.current_bytes = self.current_bytes.saturating_sub(evicted.size_bytes);
                // Advance the global inflation clock to the evicted object's priority
                self.inflation_clock = f64::from_bits(lowest.priority_bits);
            }
        }

        let priority_bits =
            self.compute_priority(1, origin_cost_micro_usd, origin_latency_ms, size_bytes);
        self.priority_tree.insert(PriorityEntry {
            priority_bits,
            key: key.clone(),
        });
        self.entries.insert(
            key,
            CachedObject {
                value,
                size_bytes,
                origin_cost_micro_usd,
                origin_latency_ms,
                frequency: 1,
                priority_key_bits: priority_bits,
                expires_at_ms,
            },
        );
        self.current_bytes += size_bytes;
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<CachedObject<V>> {
        let removed = self.entries.remove(key)?;
        self.current_bytes = self.current_bytes.saturating_sub(removed.size_bytes);
        self.priority_tree.remove(&PriorityEntry {
            priority_bits: removed.priority_key_bits,
            key: key.to_string(),
        });
        Some(removed)
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    pub fn inflation_clock(&self) -> f64 {
        self.inflation_clock
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finops_gdsf_byte_bounding_and_rejection() {
        let mut cache: FinOpsGdsfCache<String> = FinOpsGdsfCache::new(2048, 1.0);

        // Attempt item larger than total cache budget -> rejected
        assert!(!cache.insert(
            "oversized".to_string(),
            "too big".to_string(),
            4096,
            100,
            50,
            1_000_000
        ));
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.current_bytes(), 0);

        // Insert valid items within budget
        assert!(cache.insert(
            "item1".to_string(),
            "val1".to_string(),
            1000,
            500,
            100,
            1_000_000
        ));
        assert_eq!(cache.current_bytes(), 1000);

        assert!(cache.insert(
            "item2".to_string(),
            "val2".to_string(),
            800,
            200,
            50,
            1_000_000
        ));
        assert_eq!(cache.current_bytes(), 1800);
    }

    #[test]
    fn test_finops_gdsf_cost_aware_eviction_and_clock_inflation() {
        // Cache capacity: 1500 bytes
        let mut cache: FinOpsGdsfCache<String> = FinOpsGdsfCache::new(1500, 1.0);

        // High cost item: 600 bytes, cost $0.08 (80,000 micro-USD), 5000 ms latency
        cache.insert(
            "high_val".to_string(),
            "reasoning_result".to_string(),
            600,
            80_000,
            5_000,
            1_000_000,
        );

        // Low cost item: 600 bytes, cost $0.0001 (100 micro-USD), 10 ms latency
        cache.insert(
            "low_val".to_string(),
            "cheap_result".to_string(),
            600,
            100,
            10,
            1_000_000,
        );

        assert_eq!(cache.current_bytes(), 1200);

        // Third item: 500 bytes -> 1200 + 500 = 1700 > 1500 -> Eviction must occur!
        // low_val has much lower utility/size and must be evicted instead of high_val
        cache.insert(
            "item3".to_string(),
            "val3".to_string(),
            500,
            1_000,
            50,
            1_000_000,
        );

        assert!(cache.get("high_val", 100).is_some());
        assert!(cache.get("low_val", 100).is_none());
        assert!(cache.get("item3", 100).is_some());
        assert!(cache.inflation_clock() > 0.0);
    }

    #[test]
    fn test_finops_gdsf_ttl_expiry() {
        let mut cache: FinOpsGdsfCache<String> = FinOpsGdsfCache::new(2048, 1.0);
        cache.insert("temp".to_string(), "val".to_string(), 500, 100, 10, 1000);

        // Valid access before expiry
        assert_eq!(cache.get("temp", 999), Some("val".to_string()));

        // Access at or past expiry -> removed and None
        assert_eq!(cache.get("temp", 1000), None);
        assert_eq!(cache.current_bytes(), 0);
    }
}
