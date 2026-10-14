use async_trait::async_trait;
use sha2::{Digest, Sha256};
use sunborn_core::Result;

pub type Vector = Vec<f32>;

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vector>;
}

/// A high-performance deterministic embedding provider for local development,
/// testing, and headless execution. Generates 384-dimensional normalized vectors.
pub struct DeterministicEmbeddingProvider {
    pub dimension: usize,
}

impl DeterministicEmbeddingProvider {
    pub fn new() -> Self {
        Self { dimension: 384 }
    }
}

impl Default for DeterministicEmbeddingProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vector> {
        let mut vector = vec![0.0f32; self.dimension];
        let words: Vec<&str> = text.split_whitespace().collect();

        if words.is_empty() {
            return Ok(vector);
        }

        for (pos, word) in words.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(word.to_lowercase().as_bytes());
            let hash = hasher.finalize();

            // Project word hash onto vector dimensions
            for i in 0..4 {
                let idx = ((hash[i * 2] as usize) << 8 | (hash[i * 2 + 1] as usize)) % self.dimension;
                let weight = 1.0 / (1.0 + pos as f32 * 0.1);
                vector[idx] += weight;
            }
        }

        // L2 Normalize
        let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vector.iter_mut() {
                *v /= norm;
            }
        }

        Ok(vector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cosine_similarity_identical() {
        let provider = DeterministicEmbeddingProvider::new();
        let v1 = provider.embed("What is semantic caching in RAG?").await.unwrap();
        let v2 = provider.embed("What is semantic caching in RAG?").await.unwrap();

        let sim = cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn test_cosine_similarity_similar_vs_dissimilar() {
        let provider = DeterministicEmbeddingProvider::new();
        let v_base = provider.embed("What is semantic caching in RAG?").await.unwrap();
        let v_sim = provider.embed("Explain semantic caching for RAG systems").await.unwrap();
        let v_diff = provider.embed("How to bake a chocolate strawberry cake?").await.unwrap();

        let sim_close = cosine_similarity(&v_base, &v_sim);
        let sim_far = cosine_similarity(&v_base, &v_diff);

        assert!(sim_close > sim_far, "Similar queries must have higher similarity than unrelated queries: {} vs {}", sim_close, sim_far);
    }
}
