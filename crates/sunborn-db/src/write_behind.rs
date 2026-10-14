use crate::models::{AuditRecord, DurableCacheRecord};
use crate::repository::DatabaseStore;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sunborn_core::Result;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub type HydrationHook = Arc<dyn Fn(&[DurableCacheRecord]) + Send + Sync + 'static>;

pub enum WriteBehindTask {
    InsertCacheEntry(DurableCacheRecord),
    RecordAudit(AuditRecord),
    Flush(tokio::sync::oneshot::Sender<()>),
}

pub struct WriteBehindFlusher {
    tx: mpsc::Sender<WriteBehindTask>,
    enqueued_count: Arc<AtomicU64>,
    flushed_count: Arc<AtomicU64>,
    worker_handle: Option<JoinHandle<()>>,
}

impl WriteBehindFlusher {
    pub const DEFAULT_BATCH_SIZE: usize = 500;
    pub const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_millis(5);

    pub fn new<D: DatabaseStore + 'static>(
        db: Arc<D>,
        buffer_capacity: usize,
        max_batch_size: usize,
        flush_interval: Duration,
    ) -> Self {
        Self::new_with_hook(db, buffer_capacity, max_batch_size, flush_interval, None)
    }

    pub fn new_with_hook<D: DatabaseStore + 'static>(
        db: Arc<D>,
        buffer_capacity: usize,
        max_batch_size: usize,
        flush_interval: Duration,
        hydration_hook: Option<HydrationHook>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel(buffer_capacity);
        let enqueued_count = Arc::new(AtomicU64::new(0));
        let flushed_count = Arc::new(AtomicU64::new(0));
        let flushed_ref = Arc::clone(&flushed_count);

        let worker_handle = tokio::spawn(async move {
            let mut cache_batch = Vec::with_capacity(max_batch_size);
            let mut audit_batch = Vec::with_capacity(max_batch_size);
            let mut flush_notifiers = Vec::new();
            let mut interval = tokio::time::interval(flush_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        Self::drain_and_commit(
                            &db,
                            &mut cache_batch,
                            &mut audit_batch,
                            &mut flush_notifiers,
                            &flushed_ref,
                            &hydration_hook,
                        ).await;
                    }
                    task = rx.recv() => {
                        match task {
                            Some(WriteBehindTask::InsertCacheEntry(entry)) => {
                                cache_batch.push(entry);
                                if cache_batch.len() >= max_batch_size {
                                    Self::drain_and_commit(
                                        &db,
                                        &mut cache_batch,
                                        &mut audit_batch,
                                        &mut flush_notifiers,
                                        &flushed_ref,
                                        &hydration_hook,
                                    ).await;
                                }
                            }
                            Some(WriteBehindTask::RecordAudit(audit)) => {
                                audit_batch.push(audit);
                                if audit_batch.len() >= max_batch_size {
                                    Self::drain_and_commit(
                                        &db,
                                        &mut cache_batch,
                                        &mut audit_batch,
                                        &mut flush_notifiers,
                                        &flushed_ref,
                                        &hydration_hook,
                                    ).await;
                                }
                            }
                            Some(WriteBehindTask::Flush(notifier)) => {
                                flush_notifiers.push(notifier);
                                Self::drain_and_commit(
                                    &db,
                                    &mut cache_batch,
                                    &mut audit_batch,
                                    &mut flush_notifiers,
                                    &flushed_ref,
                                    &hydration_hook,
                                ).await;
                            }
                            None => {
                                // Channel closed, final drain
                                Self::drain_and_commit(
                                    &db,
                                    &mut cache_batch,
                                    &mut audit_batch,
                                    &mut flush_notifiers,
                                    &flushed_ref,
                                    &hydration_hook,
                                ).await;
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            tx,
            enqueued_count,
            flushed_count,
            worker_handle: Some(worker_handle),
        }
    }

    async fn drain_and_commit<D: DatabaseStore>(
        db: &Arc<D>,
        cache_batch: &mut Vec<DurableCacheRecord>,
        audit_batch: &mut Vec<AuditRecord>,
        flush_notifiers: &mut Vec<tokio::sync::oneshot::Sender<()>>,
        flushed_count: &Arc<AtomicU64>,
        hydration_hook: &Option<HydrationHook>,
    ) {
        if !cache_batch.is_empty() {
            let total = cache_batch.len();
            let mut committed = Vec::with_capacity(total);
            for entry in cache_batch.drain(..) {
                if db.insert_cache_entry(entry.clone()).await.is_ok() {
                    committed.push(entry);
                }
            }
            if let Some(ref hook) = hydration_hook {
                if !committed.is_empty() {
                    hook(&committed);
                }
            }
            flushed_count.fetch_add(total as u64, Ordering::Relaxed);
        }

        if !audit_batch.is_empty() {
            let total = audit_batch.len();
            for audit in audit_batch.drain(..) {
                let _ = db.record_audit_event(audit).await;
            }
            flushed_count.fetch_add(total as u64, Ordering::Relaxed);
        }

        for notifier in flush_notifiers.drain(..) {
            let _ = notifier.send(());
        }
    }

    pub async fn enqueue_cache_entry(&self, entry: DurableCacheRecord) -> Result<()> {
        self.enqueued_count.fetch_add(1, Ordering::Relaxed);
        self.tx
            .send(WriteBehindTask::InsertCacheEntry(entry))
            .await
            .map_err(|_| {
                sunborn_core::SunbornError::new(
                    sunborn_core::ErrorCode::InternalError,
                    "Write-behind ring buffer queue closed",
                )
            })
    }

    pub async fn enqueue_audit(&self, audit: AuditRecord) -> Result<()> {
        self.enqueued_count.fetch_add(1, Ordering::Relaxed);
        self.tx
            .send(WriteBehindTask::RecordAudit(audit))
            .await
            .map_err(|_| {
                sunborn_core::SunbornError::new(
                    sunborn_core::ErrorCode::InternalError,
                    "Write-behind ring buffer queue closed",
                )
            })
    }

    pub async fn flush(&self) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if self.tx.send(WriteBehindTask::Flush(tx)).await.is_ok() {
            let _ = rx.await;
        }
    }

    pub fn stats(&self) -> (u64, u64) {
        (
            self.enqueued_count.load(Ordering::Relaxed),
            self.flushed_count.load(Ordering::Relaxed),
        )
    }
}

impl Drop for WriteBehindFlusher {
    fn drop(&mut self) {
        if let Some(handle) = self.worker_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::MemoryDatabaseStore;
    use chrono::Utc;

    #[tokio::test]
    async fn test_write_behind_group_commit_batching() {
        let db = Arc::new(MemoryDatabaseStore::new());
        let flusher = WriteBehindFlusher::new(db.clone(), 2048, 100, Duration::from_millis(10));

        // Enqueue 250 records rapidly (should take < 1ms total)
        let t0 = std::time::Instant::now();
        for i in 0..250 {
            flusher
                .enqueue_cache_entry(DurableCacheRecord {
                    key: format!("k:{}", i),
                    tenant_id: "t-bench".into(),
                    application_id: "app-wb".into(),
                    model: "gpt-4o".into(),
                    policy_version: "v1".into(),
                    hash: format!("hash-{}", i),
                    prompt_canonical: format!("prompt {}", i),
                    content: format!("content {}", i),
                    hit_count: 0,
                    created_at: Utc::now(),
                    expires_at: None,
                    last_accessed_at: Utc::now(),
                })
                .await
                .unwrap();
        }
        let enqueue_elapsed = t0.elapsed();
        assert!(
            enqueue_elapsed < Duration::from_millis(50),
            "Enqueueing 250 items to ring buffer must be sub-millisecond per item"
        );

        // Trigger manual flush
        flusher.flush().await;

        let (enqueued, flushed) = flusher.stats();
        assert_eq!(enqueued, 250);
        assert_eq!(flushed, 250);
        assert_eq!(db.count_cache_entries().await.unwrap(), 250);
    }

    #[tokio::test]
    async fn test_write_behind_commit_ordered_hydration() {
        let db = Arc::new(MemoryDatabaseStore::new());
        let hydrated_count = Arc::new(AtomicU64::new(0));
        let hydrated_ref = Arc::clone(&hydrated_count);

        let hook: HydrationHook = Arc::new(move |entries| {
            hydrated_ref.fetch_add(entries.len() as u64, Ordering::Relaxed);
        });

        let flusher = WriteBehindFlusher::new_with_hook(
            db.clone(),
            100,
            10,
            Duration::from_millis(5),
            Some(hook),
        );

        flusher
            .enqueue_cache_entry(DurableCacheRecord {
                key: "k-hydrated".into(),
                tenant_id: "t-1".into(),
                application_id: "app-1".into(),
                model: "gpt-4o".into(),
                policy_version: "v1".into(),
                hash: "hash-h".into(),
                prompt_canonical: "prompt h".into(),
                content: "content h".into(),
                hit_count: 0,
                created_at: Utc::now(),
                expires_at: None,
                last_accessed_at: Utc::now(),
            })
            .await
            .unwrap();

        flusher.flush().await;

        assert_eq!(db.count_cache_entries().await.unwrap(), 1);
        assert_eq!(
            hydrated_count.load(Ordering::Relaxed),
            1,
            "Hydration hook must be triggered after DB commit"
        );
    }
}
