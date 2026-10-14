use crate::shadow::ShadowEvaluator;
use std::sync::Arc;
use sunborn_cache::{CircuitBreakerCache, ExactCacheStore, MemoryExactCache};
use sunborn_core::AppConfig;
use sunborn_db::MemoryDatabaseStore;
use sunborn_execution::MemoryExecutionCacheStore;
use sunborn_invalidation::{DependencyGraph, InvalidationEngine};
use sunborn_semantic::{
    DecisionConfig, DecisionEngine, DeterministicEmbeddingProvider, MemoryVectorStore,
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub cache: Arc<dyn ExactCacheStore>,
    pub circuit_breaker: Arc<CircuitBreakerCache<MemoryExactCache>>,
    pub memory_cache: Arc<MemoryExactCache>,
    pub vector_store: Arc<MemoryVectorStore>,
    pub decision_engine: Arc<DecisionEngine<DeterministicEmbeddingProvider, MemoryVectorStore>>,
    pub dependency_graph: Arc<DependencyGraph>,
    pub invalidation_engine: Arc<InvalidationEngine<MemoryExactCache, MemoryVectorStore>>,
    pub db: Arc<MemoryDatabaseStore>,
    pub execution_store: Arc<MemoryExecutionCacheStore>,
    pub shadow_evaluator: Arc<ShadowEvaluator>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let memory_cache = Arc::new(MemoryExactCache::new());
        let circuit_breaker = Arc::new(CircuitBreakerCache::new(
            (*memory_cache).clone(),
            3,
            std::time::Duration::from_secs(5),
        ));
        let vector_store = Arc::new(MemoryVectorStore::new());
        let db = Arc::new(MemoryDatabaseStore::new());
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
        let execution_store = Arc::new(MemoryExecutionCacheStore::new());
        let shadow_evaluator = Arc::new(ShadowEvaluator::new());

        Self {
            config,
            cache: circuit_breaker.clone(),
            circuit_breaker,
            memory_cache,
            vector_store,
            decision_engine,
            dependency_graph,
            invalidation_engine,
            db,
            execution_store,
            shadow_evaluator,
        }
    }
}
