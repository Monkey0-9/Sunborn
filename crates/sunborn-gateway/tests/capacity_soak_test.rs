use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};

#[tokio::test]
async fn test_capacity_bounded_memory_eviction() {
    let capacity_limit = 500;
    let cache = MemoryExactCache::with_capacity(capacity_limit);

    // Ingest 2,500 distinct cache entries into a 500-entry bounded cache
    let total_inserts = 2500;
    for i in 0..total_inserts {
        let key = format!("cache:key:{:05}", i);
        let content = format!("Generated synthetic payload content number {}", i);
        let entry = CacheEntry::new(key, content, Some(3600));
        cache.set(entry).await.unwrap();
    }

    // CRITICAL ACCEPTANCE CRITERIA:
    // Memory cache count must strictly equal configured capacity limit,
    // proving automatic bounded LRU/oldest eviction and zero memory leak.
    let count = cache.entry_count().await;
    assert_eq!(
        count, capacity_limit,
        "Cache entry count must flatline at capacity limit of {} without unbounded growth",
        capacity_limit
    );

    // Verify recent keys are present while early keys were evicted
    let early_key = "cache:key:00010";
    let recent_key = format!("cache:key:{:05}", total_inserts - 1);

    assert!(
        !cache.exists(early_key).await.unwrap(),
        "Early entries should be evicted"
    );
    assert!(
        cache.exists(&recent_key).await.unwrap(),
        "Recent entries must be retained"
    );
}
