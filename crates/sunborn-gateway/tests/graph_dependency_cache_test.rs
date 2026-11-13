use sunborn_cache::{CacheEntry, ExactCacheStore};
use sunborn_core::AppConfig;
use sunborn_gateway::state::AppState;
use uuid::Uuid;

#[tokio::test]
async fn test_graph_dependency_cache_feature_flag_disabled_lifecycle() {
    let mut config = AppConfig::default();
    config.cache.graph_dependency_cache_enabled = false;
    let state = AppState::new(config);

    // Feature flag is explicitly disabled: zero extra allocations or overhead
    assert!(
        state.graph_memory_cache.is_none(),
        "graph_memory_cache must be None when SUNBORN_GRAPH_DEPENDENCY_CACHE_ENABLED is false"
    );

    // Standard cache still operates normally
    let entry = CacheEntry::new("test:key:baseline", "content baseline", Some(3600));
    state.cache.set(entry).await.unwrap();
    assert!(state.cache.exists("test:key:baseline").await.unwrap());
}

#[tokio::test]
async fn test_graph_dependency_cache_active_by_default() {
    let config = AppConfig::default();
    let state = AppState::new(config);

    // Both vector and graph caching active by default
    assert!(
        state.graph_memory_cache.is_some(),
        "graph_memory_cache must be initialized by default for unified vector + graph caching"
    );
}

#[tokio::test]
async fn test_graph_dependency_cache_feature_flag_enabled_lifecycle() {
    let mut config = AppConfig::default();
    config.cache.graph_dependency_cache_enabled = true;

    let state = AppState::new(config);

    // Feature flag is enabled: graph_memory_cache is active
    assert!(
        state.graph_memory_cache.is_some(),
        "graph_memory_cache must be initialized when feature flag is enabled"
    );

    let graph_cache = state.graph_memory_cache.as_ref().unwrap();
    let source_doc_id = Uuid::new_v4();
    let doc_hash = 10086u64;

    let entry = CacheEntry::new(
        "cache:q3_financial_summary",
        "Q3 financial gross revenue exceeded expectations by 14%",
        Some(3600),
    );

    // 1. Store cache entry with explicit knowledge graph dependency lineage
    graph_cache
        .set_with_lineage(entry, &[source_doc_id], &[doc_hash], 1)
        .await
        .unwrap();

    // 2. Verify cache entry is immediately retrievable
    let retrieved = graph_cache.get("cache:q3_financial_summary").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(
        retrieved.unwrap().content,
        "Q3 financial gross revenue exceeded expectations by 14%"
    );

    // 3. Trigger Real-Time Causal (RTC) Invalidation for source_doc_id
    let purged = graph_cache
        .invalidate_source_rtc(&source_doc_id)
        .await
        .unwrap();
    assert_eq!(purged, vec!["cache:q3_financial_summary".to_string()]);

    // 4. Assert entry is immediately evicted from memory without delay
    let after_inval = graph_cache.get("cache:q3_financial_summary").await.unwrap();
    assert!(
        after_inval.is_none(),
        "Causally dependent cache entry must be immediately purged upon upstream document mutation"
    );
}
