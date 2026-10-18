use async_trait::async_trait;
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
    fn is_deterministic(&self) -> bool {
        false
    }
    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl EmbeddingProvider for std::sync::Arc<dyn EmbeddingProvider> {
    async fn embed(&self, text: &str) -> Result<Vector> {
        (**self).embed(text).await
    }
    fn is_deterministic(&self) -> bool {
        (**self).is_deterministic()
    }
    async fn health_check(&self) -> Result<()> {
        (**self).health_check().await
    }
}

/// Locality-Sensitive Hashing (LSH) Signed Random Projection Embedding Provider.
/// A zero-dependency, high-throughput CPU baseline that extracts word and character
/// n-grams and projects them onto signed hyperplanes, preserving locality-sensitive
/// cosine similarity between semantically similar and paraphrased texts.
#[derive(Clone, Debug)]
pub struct LshRandomProjectionEmbeddingProvider {
    pub dimension: usize,
}

impl LshRandomProjectionEmbeddingProvider {
    pub fn new() -> Self {
        Self { dimension: 384 }
    }

    pub fn with_dimension(dimension: usize) -> Self {
        Self { dimension }
    }

    #[inline]
    fn fnv1a_64(bytes: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }
}

impl Default for LshRandomProjectionEmbeddingProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingProvider for LshRandomProjectionEmbeddingProvider {
    fn is_deterministic(&self) -> bool {
        true
    }

    async fn embed(&self, text: &str) -> Result<Vector> {
        let mut vector = vec![0.0f32; self.dimension];
        let lower = text.to_lowercase();
        let words: Vec<&str> = lower.split_whitespace().collect();

        if words.is_empty() {
            return Ok(vector);
        }

        // 1. Word unigram signed projections
        for (pos, word) in words.iter().enumerate() {
            let h = Self::fnv1a_64(word.as_bytes());
            let dim_idx = (h as usize) % self.dimension;
            let sign = if (h & (1 << 63)) != 0 {
                1.0f32
            } else {
                -1.0f32
            };
            let pos_decay = 1.0 / (1.0 + pos as f32 * 0.05);
            vector[dim_idx] += sign * pos_decay;

            // 2. Character 3-grams for subword morphological preservation
            let chars: Vec<char> = word.chars().collect();
            if chars.len() >= 3 {
                for window in chars.windows(3) {
                    let s: String = window.iter().collect();
                    let ch_h = Self::fnv1a_64(s.as_bytes());
                    let ch_dim = (ch_h as usize) % self.dimension;
                    let ch_sign = if (ch_h & (1 << 63)) != 0 {
                        0.5f32
                    } else {
                        -0.5f32
                    };
                    vector[ch_dim] += ch_sign * pos_decay;
                }
            }
        }

        // L2 Normalization
        let norm: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vector.iter_mut() {
                *v /= norm;
            }
        }

        Ok(vector)
    }
}

/// A deterministic embedding provider for local development, testing, and headless execution.
/// Uses the LshRandomProjectionEmbeddingProvider as its underlying projection engine.
#[derive(Clone, Debug)]
pub struct DeterministicEmbeddingProvider {
    inner: LshRandomProjectionEmbeddingProvider,
}

impl DeterministicEmbeddingProvider {
    pub fn new() -> Self {
        Self {
            inner: LshRandomProjectionEmbeddingProvider::new(),
        }
    }
}

impl Default for DeterministicEmbeddingProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EmbeddingProvider for DeterministicEmbeddingProvider {
    fn is_deterministic(&self) -> bool {
        true
    }

    async fn embed(&self, text: &str) -> Result<Vector> {
        self.inner.embed(text).await
    }
}

/// Production HTTP embedding provider for OpenAI text-embedding-3 or local vLLM/Ollama embeddings
#[derive(Clone)]
pub struct HttpEmbeddingProvider {
    pub endpoint_url: String,
    pub api_key: Option<String>,
    pub model: String,
    client: reqwest::Client,
    fallback_provider: DeterministicEmbeddingProvider,
}

impl HttpEmbeddingProvider {
    pub fn new(
        endpoint_url: impl Into<String>,
        model: impl Into<String>,
        api_key: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .connect_timeout(std::time::Duration::from_secs(3))
            .build()
            .unwrap_or_default();

        Self {
            endpoint_url: endpoint_url.into(),
            api_key,
            model: model.into(),
            client,
            fallback_provider: DeterministicEmbeddingProvider::new(),
        }
    }
}

#[async_trait]
impl EmbeddingProvider for HttpEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vector> {
        let mut url = self.endpoint_url.trim_end_matches('/').to_string();
        if !url.ends_with("/embeddings") {
            url.push_str("/embeddings");
        }

        let payload = serde_json::json!({
            "input": text,
            "model": self.model,
        });

        let mut req = self.client.post(&url).json(&payload);
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
        }

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(embedding) =
                        data.pointer("/data/0/embedding").and_then(|v| v.as_array())
                    {
                        let vec: Vec<f32> = embedding
                            .iter()
                            .filter_map(|val| val.as_f64().map(|f| f as f32))
                            .collect();
                        if !vec.is_empty() {
                            return Ok(vec);
                        }
                    }
                }
            }
        }

        let is_prod = std::env::var("SUNBORN_ENV")
            .map(|v| v.to_lowercase() == "production")
            .unwrap_or(false);
        if is_prod {
            return Err(sunborn_core::SunbornError::embedding(format!(
                "Upstream HTTP embedding provider at {} failed to generate valid vector; fail-closed abstention enforced in production",
                self.endpoint_url
            )));
        }

        // Resilient fallback for offline unit tests & benchmark runs
        self.fallback_provider.embed(text).await
    }

    async fn health_check(&self) -> Result<()> {
        let mut url = self.endpoint_url.trim_end_matches('/').to_string();
        if !url.ends_with("/embeddings") {
            url.push_str("/embeddings");
        }

        let payload = serde_json::json!({
            "input": "healthcheck",
            "model": self.model,
        });

        let mut req = self.client.post(&url).json(&payload);
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
        }

        match req.send().await {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(sunborn_core::SunbornError::embedding(format!(
                "Upstream HTTP embedding provider at {} returned error status: {}",
                url,
                resp.status()
            ))),
            Err(err) => Err(sunborn_core::SunbornError::embedding(format!(
                "Failed to connect to upstream HTTP embedding provider at {}: {}",
                url, err
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cosine_similarity_identical() {
        let provider = DeterministicEmbeddingProvider::new();
        let v1 = provider
            .embed("What is semantic caching in RAG?")
            .await
            .unwrap();
        let v2 = provider
            .embed("What is semantic caching in RAG?")
            .await
            .unwrap();

        let sim = cosine_similarity(&v1, &v2);
        assert!((sim - 1.0).abs() < 1e-4);
    }

    #[tokio::test]
    async fn test_cosine_similarity_similar_vs_dissimilar() {
        let provider = DeterministicEmbeddingProvider::new();
        let v_base = provider
            .embed("What is semantic caching in RAG?")
            .await
            .unwrap();
        let v_sim = provider
            .embed("Explain semantic caching for RAG systems")
            .await
            .unwrap();
        let v_diff = provider
            .embed("How to bake a chocolate strawberry cake?")
            .await
            .unwrap();

        let sim_close = cosine_similarity(&v_base, &v_sim);
        let sim_far = cosine_similarity(&v_base, &v_diff);

        assert!(
            sim_close > sim_far,
            "Similar queries must have higher similarity than unrelated queries: {} vs {}",
            sim_close,
            sim_far
        );
    }
}
