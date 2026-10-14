use bytes::Bytes;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

type InFlightMap = Arc<RwLock<HashMap<[u8; 32], broadcast::Sender<Result<Bytes, String>>>>>;

/// In-Flight Stream Multiplexer (SingleflightSSE)
/// Coalesces concurrent cache misses on identical prompts so only 1 leader
/// request calls the upstream origin LLM, while all followers receive
/// live streaming chunks simultaneously.
#[derive(Clone)]
pub struct SingleflightStreamer {
    in_flight: InFlightMap,
    leaders_spawned: Arc<AtomicU64>,
    followers_attached: Arc<AtomicU64>,
}

impl Default for SingleflightStreamer {
    fn default() -> Self {
        Self::new()
    }
}

impl SingleflightStreamer {
    pub fn new() -> Self {
        Self {
            in_flight: Arc::new(RwLock::new(HashMap::new())),
            leaders_spawned: Arc::new(AtomicU64::new(0)),
            followers_attached: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Attaches to an ongoing stream if another request is already in-flight,
    /// or registers this request as leader and spawns the upstream origin task.
    /// Returns: (Receiver for stream chunks, is_leader boolean)
    pub async fn subscribe_or_spawn<F, Fut>(
        &self,
        identity_hash: [u8; 32],
        origin_task: F,
    ) -> (broadcast::Receiver<Result<Bytes, String>>, bool)
    where
        F: FnOnce(broadcast::Sender<Result<Bytes, String>>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // 1. Fast path: check if already in flight
        {
            let map = self.in_flight.read().await;
            if let Some(existing_tx) = map.get(&identity_hash) {
                self.followers_attached.fetch_add(1, Ordering::Relaxed);
                return (existing_tx.subscribe(), false); // Follower attaches to live SSE stream
            }
        }

        // 2. Slow path: acquire write lock to insert new channel
        let mut map = self.in_flight.write().await;
        if let Some(existing_tx) = map.get(&identity_hash) {
            self.followers_attached.fetch_add(1, Ordering::Relaxed);
            return (existing_tx.subscribe(), false);
        }

        let (tx, rx) = broadcast::channel(256);
        map.insert(identity_hash, tx.clone());
        self.leaders_spawned.fetch_add(1, Ordering::Relaxed);

        let map_ref = Arc::clone(&self.in_flight);
        tokio::spawn(async move {
            origin_task(tx).await;
            let mut map = map_ref.write().await;
            map.remove(&identity_hash);
        });

        (rx, true) // Leader executes upstream LLM request
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.leaders_spawned.load(Ordering::Relaxed),
            self.followers_attached.load(Ordering::Relaxed),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_singleflight_streamer_multiplexing() {
        let streamer = SingleflightStreamer::new();
        let hash = [7u8; 32];

        // 1. Leader request spawns slow upstream origin stream
        let (mut leader_rx, is_leader) = streamer
            .subscribe_or_spawn(hash, |tx| async move {
                for i in 1..=4 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    let chunk = Bytes::from(format!("chunk_{}", i));
                    let _ = tx.send(Ok(chunk));
                }
            })
            .await;

        assert!(is_leader);

        // 2. Follower requests attach to the ongoing stream
        let (mut follower_rx_1, is_leader_1) =
            streamer.subscribe_or_spawn(hash, |_| async {}).await;
        assert!(!is_leader_1, "Follower 1 must not be leader");

        let (mut follower_rx_2, is_leader_2) =
            streamer.subscribe_or_spawn(hash, |_| async {}).await;
        assert!(!is_leader_2, "Follower 2 must not be leader");

        // 3. Verify leader and followers receive identical chunks concurrently
        let leader_first = leader_rx.recv().await.unwrap().unwrap();
        let follower_first_1 = follower_rx_1.recv().await.unwrap().unwrap();
        let follower_first_2 = follower_rx_2.recv().await.unwrap().unwrap();

        assert_eq!(leader_first, Bytes::from("chunk_1"));
        assert_eq!(follower_first_1, Bytes::from("chunk_1"));
        assert_eq!(follower_first_2, Bytes::from("chunk_1"));

        let (leaders, followers) = streamer.stats();
        assert_eq!(leaders, 1);
        assert_eq!(followers, 2);
    }
}
