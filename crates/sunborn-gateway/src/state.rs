use std::sync::Arc;
use sunborn_cache::{ExactCacheStore, MemoryExactCache};
use sunborn_core::AppConfig;
use sunborn_invalidation::{DependencyGraph, InvalidationEngine};
use sunborn_semantic::{
    DecisionConfig, DecisionEngine, DeterministicEmbeddingProvider, MemoryVectorStore,
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub cache: Arc<dyn ExactCacheStore>,
    pub memory_cache: Arc<MemoryExactCache>,
    pub vector_store: Arc<MemoryVectorStore>,
    pub decision_engine: Arc<DecisionEngine<DeterministicEmbeddingProvider, MemoryVectorStore>>,
    pub dependency_graph: Arc<DependencyGraph>,
    pub invalidation_engine: Arc<InvalidationEngine<MemoryExactCache, MemoryVectorStore>>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let memory_cache = Arc::new(MemoryExactCache::new());
        let vector_store = Arc::new(MemoryVectorStore::new());
        let embedder = DeterministicEmbeddingProvider::new();
        let decision_engine = Arc::new(DecisionEngine::new(
            embedder,
            (*vector_store).clone(),
            DecisionConfig::default(),
        ));
        let dependency_graph = Arc::new(DependencyGraph::new());
        let invalidation_engine = Arc::new(InvalidationEngine::new(
            dependency_graph.clone(),
            memory_cache.clone(),
            vector_store.clone(),
        ));

        Self {
            config,
            cache: memory_cache.clone(),
            memory_cache,
            vector_store,
            decision_engine,
            dependency_graph,
            invalidation_engine,
        }
    }
}
