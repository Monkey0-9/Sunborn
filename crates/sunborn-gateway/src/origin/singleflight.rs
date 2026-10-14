use bytes::Bytes;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

#[derive(Clone)]
struct InFlightStream {
    tx: broadcast::Sender<Result<Bytes, String>>,
    history: Arc<RwLock<Vec<Bytes>>>,
}

type InFlightMap = Arc<RwLock<HashMap<[u8; 32], InFlightStream>>>;

/// Replay-capable receiver that catches up from historical chunks if a follower
/// connects mid-stream or experiences `RecvError::Lagged`.
pub struct ReplayStreamReceiver {
    history: Arc<RwLock<Vec<Bytes>>>,
    receiver: broadcast::Receiver<Result<Bytes, String>>,
    cursor: usize,
}

impl ReplayStreamReceiver {
    pub fn new(
        history: Arc<RwLock<Vec<Bytes>>>,
        receiver: broadcast::Receiver<Result<Bytes, String>>,
    ) -> Self {
        Self {
            history,
            receiver,
            cursor: 0,
        }
    }

    /// Receives the next chunk, transparently replaying missed slices from the
    /// chunk history log if broadcast lagging or late attachment occurs.
    pub async fn recv(&mut self) -> Result<Result<Bytes, String>, broadcast::error::RecvError> {
        // 1. Drain any pending historical chunks first
        {
            let hist = self.history.read().await;
            if self.cursor < hist.len() {
                let chunk = hist[self.cursor].clone();
                self.cursor += 1;
                return Ok(Ok(chunk));
            }
        }

        // 2. Receive live chunk from broadcast channel
        loop {
            match self.receiver.recv().await {
                Ok(item) => {
                    self.cursor += 1;
                    return Ok(item);
                }
                Err(broadcast::error::RecvError::Lagged(_skipped)) => {
                    // Stalled or slow follower lagged behind broadcast buffer!
                    // Seamlessly replay missed chunks directly from the in-memory history log
                    let hist = self.history.read().await;
                    if self.cursor < hist.len() {
                        let chunk = hist[self.cursor].clone();
                        self.cursor += 1;
                        return Ok(Ok(chunk));
                    }
                    // If no further history exists, continue waiting for next live item
                }
                Err(broadcast::error::RecvError::Closed) => {
                    // Check if any remaining chunks were added before closure
                    let hist = self.history.read().await;
                    if self.cursor < hist.len() {
                        let chunk = hist[self.cursor].clone();
                        self.cursor += 1;
                        return Ok(Ok(chunk));
                    }
                    return Err(broadcast::error::RecvError::Closed);
                }
            }
        }
    }
}

/// A wrapper around broadcast::Sender that records chunks into the history log.
#[derive(Clone)]
pub struct SingleflightSender {
    tx: broadcast::Sender<Result<Bytes, String>>,
    history: Arc<RwLock<Vec<Bytes>>>,
}

impl SingleflightSender {
    pub async fn send(&self, item: Result<Bytes, String>) {
        if let Ok(ref bytes) = item {
            let mut hist = self.history.write().await;
            hist.push(bytes.clone());
        }
        let _ = self.tx.send(item);
    }
}

/// In-Flight Stream Multiplexer (SingleflightSSE)
/// Coalesces concurrent cache misses on identical prompts so only 1 leader
/// request calls the upstream origin LLM, while all followers receive
/// live streaming chunks simultaneously with lag protection.
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
    /// Returns: (ReplayStreamReceiver for stream chunks, is_leader boolean)
    pub async fn subscribe_or_spawn<F, Fut>(
        &self,
        identity_hash: [u8; 32],
        origin_task: F,
    ) -> (ReplayStreamReceiver, bool)
    where
        F: FnOnce(SingleflightSender) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // 1. Fast path: check if already in flight
        {
            let map = self.in_flight.read().await;
            if let Some(stream) = map.get(&identity_hash) {
                self.followers_attached.fetch_add(1, Ordering::Relaxed);
                return (
                    ReplayStreamReceiver::new(Arc::clone(&stream.history), stream.tx.subscribe()),
                    false,
                );
            }
        }

        // 2. Slow path: acquire write lock to insert new channel
        let mut map = self.in_flight.write().await;
        if let Some(stream) = map.get(&identity_hash) {
            self.followers_attached.fetch_add(1, Ordering::Relaxed);
            return (
                ReplayStreamReceiver::new(Arc::clone(&stream.history), stream.tx.subscribe()),
                false,
            );
        }

        let (tx, rx) = broadcast::channel(256);
        let history = Arc::new(RwLock::new(Vec::new()));
        let stream_entry = InFlightStream {
            tx: tx.clone(),
            history: Arc::clone(&history),
        };
        map.insert(identity_hash, stream_entry);
        self.leaders_spawned.fetch_add(1, Ordering::Relaxed);

        let map_ref = Arc::clone(&self.in_flight);
        let sender = SingleflightSender {
            tx,
            history: Arc::clone(&history),
        };

        tokio::spawn(async move {
            origin_task(sender).await;
            let mut map = map_ref.write().await;
            map.remove(&identity_hash);
        });

        (ReplayStreamReceiver::new(history, rx), true)
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

        // Spawn leader task emitting 5 chunks with delay
        let (mut leader_rx, is_leader) = streamer
            .subscribe_or_spawn(hash, |sender| async move {
                for i in 1..=5 {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    sender.send(Ok(Bytes::from(format!("chunk-{i}")))).await;
                }
            })
            .await;

        assert!(is_leader, "First request must be elected leader");

        // Concurrently attach 2 followers
        let (mut follower_1, is_f1_leader) = streamer.subscribe_or_spawn(hash, |_| async {}).await;
        assert!(!is_f1_leader, "Follower 1 must not be leader");

        let (mut follower_2, is_f2_leader) = streamer.subscribe_or_spawn(hash, |_| async {}).await;
        assert!(!is_f2_leader, "Follower 2 must not be leader");

        // Collect all chunks across leader and followers
        let mut leader_chunks = Vec::new();
        while let Ok(Ok(chunk)) = leader_rx.recv().await {
            leader_chunks.push(String::from_utf8(chunk.to_vec()).unwrap());
            if leader_chunks.len() == 5 {
                break;
            }
        }

        let mut f1_chunks = Vec::new();
        while let Ok(Ok(chunk)) = follower_1.recv().await {
            f1_chunks.push(String::from_utf8(chunk.to_vec()).unwrap());
            if f1_chunks.len() == 5 {
                break;
            }
        }

        let mut f2_chunks = Vec::new();
        while let Ok(Ok(chunk)) = follower_2.recv().await {
            f2_chunks.push(String::from_utf8(chunk.to_vec()).unwrap());
            if f2_chunks.len() == 5 {
                break;
            }
        }

        assert_eq!(
            leader_chunks,
            vec!["chunk-1", "chunk-2", "chunk-3", "chunk-4", "chunk-5"]
        );
        assert_eq!(leader_chunks, f1_chunks);
        assert_eq!(leader_chunks, f2_chunks);

        let (leaders, followers) = streamer.stats();
        assert_eq!(leaders, 1);
        assert_eq!(followers, 2);
    }

    #[tokio::test]
    async fn test_singleflight_slow_follower_lag_recovery() {
        let streamer = SingleflightStreamer::new();
        let hash = [42u8; 32];

        let (done_tx, done_rx) = tokio::sync::oneshot::channel();

        // Leader emits 10 chunks quickly and keeps stream open until follower reads
        let (_leader_rx, is_leader) = streamer
            .subscribe_or_spawn(hash, |sender| async move {
                for i in 1..=10 {
                    sender.send(Ok(Bytes::from(format!("token-{i}")))).await;
                }
                let _ = done_rx.await;
            })
            .await;
        assert!(is_leader);

        // Allow leader to emit chunks into history
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Slow or late-attaching follower connects while in-flight
        let (mut slow_follower, is_slow_leader) =
            streamer.subscribe_or_spawn(hash, |_| async {}).await;
        assert!(!is_slow_leader);

        let _ = done_tx.send(());

        let mut received = Vec::new();
        while let Ok(Ok(chunk)) = slow_follower.recv().await {
            received.push(String::from_utf8(chunk.to_vec()).unwrap());
            if received.len() == 10 {
                break;
            }
        }

        assert_eq!(
            received.len(),
            10,
            "Slow follower must recover all chunks from history"
        );
        assert_eq!(received[0], "token-1");
        assert_eq!(received[9], "token-10");
    }
}
