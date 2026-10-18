// ============================================================================
// crates/sunborn-semantic/src/context_digest.rs
// Context-Lineage Binding (ContextDigest) for Tier C2 Semantic Verification
// Binds model_id, system prompt hash, and RAG document chunk IDs Merkle root.
// Two prompts are compared for semantic reuse iff their ContextDigest matches.
// ============================================================================
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContextDigest([u8; 32]);

impl ContextDigest {
    pub fn new(raw: [u8; 32]) -> Self {
        Self(raw)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn from_hex(s: &str) -> std::result::Result<Self, String> {
        let bytes = hex::decode(s).map_err(|e| format!("Invalid hex for ContextDigest: {e}"))?;
        if bytes.len() != 32 {
            return Err(format!(
                "ContextDigest requires exactly 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Computes the 32-byte ContextDigest over:
    /// - model_id
    /// - system_prompt_hash (SHA-256 of system prompt or zeroes if None)
    /// - rag_chunk_ids_merkle_root (Merkle root over lexicographically sorted RAG chunk identifiers)
    pub fn compute(model_id: &str, system_prompt: Option<&str>, rag_chunk_ids: &[&str]) -> Self {
        let system_hash = match system_prompt {
            Some(p) => {
                let mut h = Sha256::new();
                h.update(b"sunborn:sys_prompt:v1:");
                h.update(p.as_bytes());
                h.finalize()
            }
            None => {
                let mut h = Sha256::new();
                h.update(b"sunborn:sys_prompt:v1:none");
                h.finalize()
            }
        };

        // Compute Merkle root over sorted RAG chunk IDs
        let mut sorted_chunks = rag_chunk_ids.to_vec();
        sorted_chunks.sort();
        let rag_root = if sorted_chunks.is_empty() {
            let mut h = Sha256::new();
            h.update(b"sunborn:rag_root:empty");
            h.finalize()
        } else {
            let mut current_hashes: Vec<Vec<u8>> = sorted_chunks
                .iter()
                .map(|chunk| {
                    let mut h = Sha256::new();
                    h.update(b"sunborn:rag_leaf:");
                    h.update(chunk.as_bytes());
                    h.finalize().to_vec()
                })
                .collect();

            while current_hashes.len() > 1 {
                let mut next_level = Vec::with_capacity(current_hashes.len().div_ceil(2));
                for chunk in current_hashes.chunks(2) {
                    let mut h = Sha256::new();
                    h.update(b"sunborn:rag_node:");
                    h.update(&chunk[0]);
                    if chunk.len() > 1 {
                        h.update(&chunk[1]);
                    } else {
                        h.update(&chunk[0]); // Duplicate odd terminal node
                    }
                    next_level.push(h.finalize().to_vec());
                }
                current_hashes = next_level;
            }
            let mut out = [0u8; 32];
            out.copy_from_slice(&current_hashes[0]);
            sha2::digest::Output::<Sha256>::from(out)
        };

        // Combine (model_id, system_hash, rag_root) into ContextDigest
        let mut final_hasher = Sha256::new();
        final_hasher.update(b"sunborn:context_digest:v1:");
        final_hasher.update((model_id.len() as u32).to_be_bytes());
        final_hasher.update(model_id.as_bytes());
        final_hasher.update(system_hash);
        final_hasher.update(rag_root);

        let digest = final_hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&digest);
        Self(arr)
    }
}

impl fmt::Debug for ContextDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ContextDigest({})", self.to_hex())
    }
}

impl fmt::Display for ContextDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_digest_determinism_and_sensitivity() {
        let d1 = ContextDigest::compute("gpt-4o", Some("System prompt A"), &["doc-1", "doc-2"]);
        let d2 = ContextDigest::compute("gpt-4o", Some("System prompt A"), &["doc-2", "doc-1"]); // sorted order invariant
        assert_eq!(d1, d2, "Order of RAG chunks must not affect ContextDigest");

        // Different system prompt -> different digest
        let d3 = ContextDigest::compute("gpt-4o", Some("System prompt B"), &["doc-1", "doc-2"]);
        assert_ne!(d1, d3);

        // Different model -> different digest
        let d4 = ContextDigest::compute(
            "claude-3-5-sonnet",
            Some("System prompt A"),
            &["doc-1", "doc-2"],
        );
        assert_ne!(d1, d4);

        // Different RAG chunks -> different digest
        let d5 = ContextDigest::compute("gpt-4o", Some("System prompt A"), &["doc-1", "doc-3"]);
        assert_ne!(d1, d5);
    }

    #[test]
    fn test_context_digest_hex_roundtrip() {
        let d = ContextDigest::compute("gpt-4o", None, &[]);
        let hex_str = d.to_hex();
        let parsed = ContextDigest::from_hex(&hex_str).unwrap();
        assert_eq!(d, parsed);
    }
}
