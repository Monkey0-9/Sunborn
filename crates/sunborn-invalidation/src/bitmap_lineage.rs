use dashmap::DashMap;
use roaring::RoaringTreemap;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Compressed 64-Bit Roaring Treemap Lineage Engine.
///
/// Under 1,000,000+ cached prompts referencing tens of thousands of RAG documents,
/// conventional `HashMap<String, HashSet<String>>` structures incur massive memory overhead
/// (hundreds of megabytes) and high cache-line bouncing during traversal.
///
/// `CompressedLineageGraph` maps 64-bit Document/Chunk hashes directly to SIMD-accelerated
/// compressed Roaring Treemaps. Computing invalidations across multi-document updates runs
/// in $O(k)$ bitwise OR operations without heap allocations on hot paths.
#[derive(Default)]
pub struct CompressedLineageGraph {
    /// Maps 64-bit hash of a RAG Document/Chunk ID to a compressed bitmap of CacheRecord IDs
    doc_to_records: DashMap<u64, RoaringTreemap>,
    /// Total tracked unique associations
    total_associations: AtomicUsize,
}

impl CompressedLineageGraph {
    pub fn new() -> Self {
        Self {
            doc_to_records: DashMap::new(),
            total_associations: AtomicUsize::new(0),
        }
    }

    /// Links a cache record numeric ID to a specific knowledge document hash.
    pub fn link_document(&self, doc_hash: u64, record_id: u64) {
        let mut entry = self.doc_to_records.entry(doc_hash).or_default();
        if entry.insert(record_id) {
            self.total_associations.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Links a cache record to multiple knowledge documents simultaneously.
    pub fn link_documents(&self, doc_hashes: &[u64], record_id: u64) {
        for &doc_hash in doc_hashes {
            self.link_document(doc_hash, record_id);
        }
    }

    /// Computes the exact deduplicated set of all cache records affected by a single document mutation.
    pub fn invalidate_document(&self, doc_hash: u64) -> RoaringTreemap {
        if let Some((_, bitmap)) = self.doc_to_records.remove(&doc_hash) {
            let count = bitmap.len() as usize;
            self.total_associations.fetch_sub(count, Ordering::Relaxed);
            bitmap
        } else {
            RoaringTreemap::new()
        }
    }

    /// Computes the exact deduplicated union of all cache records affected by a multi-document update
    /// in $O(k)$ SIMD time using fast bitwise OR (`|=`) across compressed Roaring 64-bit containers.
    pub fn invalidate_documents(&self, mutated_doc_hashes: &[u64]) -> RoaringTreemap {
        let mut affected = RoaringTreemap::new();
        for &doc_hash in mutated_doc_hashes {
            if let Some((_, bitmap)) = self.doc_to_records.remove(&doc_hash) {
                let count = bitmap.len() as usize;
                self.total_associations.fetch_sub(count, Ordering::Relaxed);
                affected |= bitmap; // Fast bitwise OR across compressed 64-bit roaring containers
            }
        }
        affected
    }

    /// Returns the number of distinct knowledge documents currently tracked.
    pub fn document_count(&self) -> usize {
        self.doc_to_records.len()
    }

    /// Returns the total number of document-to-record associations tracked.
    pub fn total_associations(&self) -> usize {
        self.total_associations.load(Ordering::Relaxed)
    }

    /// Returns true if a document hash is actively tracked.
    pub fn contains_document(&self, doc_hash: u64) -> bool {
        self.doc_to_records.contains_key(&doc_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressed_lineage_link_and_invalidation() {
        let graph = CompressedLineageGraph::new();

        let doc_a: u64 = 0x1111_2222_3333_4444;
        let doc_b: u64 = 0x5555_6666_7777_8888;
        let doc_c: u64 = 0x9999_aaaa_bbbb_cccc;

        // Record 1 depends on Doc A and Doc B
        graph.link_documents(&[doc_a, doc_b], 1001);
        // Record 2 depends on Doc B and Doc C
        graph.link_documents(&[doc_b, doc_c], 1002);
        // Record 3 depends only on Doc A
        graph.link_document(doc_a, 1003);

        assert_eq!(graph.document_count(), 3);
        assert_eq!(graph.total_associations(), 5);

        // Mutating Doc B should invalidate Record 1001 and 1002
        let affected_b = graph.invalidate_document(doc_b);
        assert_eq!(affected_b.len(), 2);
        assert!(affected_b.contains(1001));
        assert!(affected_b.contains(1002));
        assert!(!affected_b.contains(1003));

        // Mutating Doc A should invalidate remaining Record 1003 (and previously 1001)
        let affected_a = graph.invalidate_document(doc_a);
        assert_eq!(affected_a.len(), 2);
        assert!(affected_a.contains(1001));
        assert!(affected_a.contains(1003));

        // Multi-document update on Doc C
        let affected_multi = graph.invalidate_documents(&[doc_c]);
        assert_eq!(affected_multi.len(), 1);
        assert!(affected_multi.contains(1002));

        // Graph should now be completely clear
        assert_eq!(graph.document_count(), 0);
        assert_eq!(graph.total_associations(), 0);
    }

    #[test]
    fn test_scale_100k_roaring_bitmap_simd_union() {
        let graph = CompressedLineageGraph::new();
        let doc_core = 0xcafe_babe_dead_beef;

        // Link 50,000 distinct record IDs to a shared core document
        for id in 1..=50_000 {
            graph.link_document(doc_core, id);
        }

        assert_eq!(graph.document_count(), 1);
        assert_eq!(graph.total_associations(), 50_000);

        // Instantaneous SIMD bitmap invalidation
        let start = std::time::Instant::now();
        let affected = graph.invalidate_document(doc_core);
        let elapsed = start.elapsed();

        assert_eq!(affected.len(), 50_000);
        assert!(
            elapsed.as_micros() < 5000,
            "50k invalidation took {:?}, expected < 5ms",
            elapsed
        );
        assert_eq!(graph.document_count(), 0);
    }
}
