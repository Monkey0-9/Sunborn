use crate::embedding::{EmbeddingProvider, Vector};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sunborn_core::Result;
use tokio::sync::{mpsc, oneshot};

struct BatchItem {
    text: String,
    responder: oneshot::Sender<Result<Vector>>,
}

#[derive(Clone)]
pub struct MicroBatchedEmbedder {
    sender: mpsc::Sender<BatchItem>,
    total_requests: Arc<AtomicU64>,
    total_batches: Arc<AtomicU64>,
}

impl MicroBatchedEmbedder {
    pub const DEFAULT_MAX_BATCH_SIZE: usize = 32;
    pub const DEFAULT_COALESCE_WINDOW: Duration = Duration::from_micros(1500); // 1.5ms

    pub fn new<E: EmbeddingProvider + 'static>(
        embedder: E,
        max_batch_size: usize,
        coalesce_window: Duration,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel::<BatchItem>(1024);
        let total_requests = Arc::new(AtomicU64::new(0));
        let total_batches = Arc::new(AtomicU64::new(0));
        let batches_counter = Arc::clone(&total_batches);

        // Dedicated background worker task isolated from the request hot path
        tokio::spawn(async move {
            let mut pending: Vec<BatchItem> = Vec::with_capacity(max_batch_size);

            loop {
                // Wait for the first item
                let first = match rx.recv().await {
                    Some(item) => item,
                    None => break, // Channel closed
                };
                pending.push(first);

                // Start coalescing timer for remaining items in the micro-batch window
                let deadline = tokio::time::Instant::now() + coalesce_window;
                while pending.len() < max_batch_size {
                    tokio::select! {
                        biased;
                        item = rx.recv() => {
                            match item {
                                Some(it) => pending.push(it),
                                None => break,
                            }
                        }
                        _ = tokio::time::sleep_until(deadline) => {
                            break;
                        }
                    }
                }

                batches_counter.fetch_add(1, Ordering::Relaxed);
                let current_batch = std::mem::take(&mut pending);

                // Process the micro-batch
                for item in current_batch {
                    let res = embedder.embed(&item.text).await;
                    let _ = item.responder.send(res);
                }
            }
        });

        Self {
            sender: tx,
            total_requests,
            total_batches,
        }
    }

    pub async fn embed(&self, text: &str) -> Result<Vector> {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(BatchItem {
                text: text.to_string(),
                responder: tx,
            })
            .await
            .map_err(|_| {
                sunborn_core::SunbornError::new(
                    sunborn_core::ErrorCode::InternalError,
                    "Micro-batch inference queue closed",
                )
            })?;

        rx.await.map_err(|_| {
            sunborn_core::SunbornError::new(
                sunborn_core::ErrorCode::InternalError,
                "Micro-batch worker dropped inference response",
            )
        })?
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.total_requests.load(Ordering::Relaxed),
            self.total_batches.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::DeterministicEmbeddingProvider;

    #[tokio::test]
    async fn test_micro_batched_embedder_coalescing() {
        let embedder = DeterministicEmbeddingProvider::new();
        let batched = MicroBatchedEmbedder::new(embedder, 16, Duration::from_millis(5));

        // Spawn 10 concurrent embed requests
        let mut handles = Vec::new();
        for i in 0..10 {
            let b = batched.clone();
            let handle = tokio::spawn(async move {
                let prompt = format!("Micro-batch test prompt {}", i);
                b.embed(&prompt).await
            });
            handles.push(handle);
        }

        for h in handles {
            let res = h.await.unwrap().unwrap();
            assert_eq!(res.len(), 384);
        }

        let (reqs, batches) = batched.stats();
        assert_eq!(reqs, 10);
        assert!(
            batches <= 3,
            "10 concurrent requests should coalesce into <= 3 batches"
        );
    }
}
