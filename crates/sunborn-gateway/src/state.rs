use crate::autonomic::AutonomicSelfHealingController;
use crate::routing::predictive::PredictivePrefetchEngine;
use crate::routing::radix::KvCacheRouter;
use crate::shadow::ShadowEvaluator;
use std::sync::{Arc, RwLock};
use sunborn_cache::{CircuitBreakerCache, ExactCacheStore, MemoryExactCache};
use sunborn_core::cluster::VectorClock;
use sunborn_core::AppConfig;
use sunborn_db::MemoryDatabaseStore;
use sunborn_execution::MemoryExecutionCacheStore;
use sunborn_identity::crypto_shred::TenantCryptoRegistry;
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
    pub radix_router: Arc<KvCacheRouter>,
    pub crypto_registry: Arc<TenantCryptoRegistry>,
    pub predictive_engine: Arc<PredictivePrefetchEngine>,
    pub autonomic_controller: Arc<AutonomicSelfHealingController>,
    pub vector_clock: Arc<RwLock<VectorClock>>,
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
        let radix_router = Arc::new(KvCacheRouter::new(
            vec![
                "inference-node-1".to_string(),
                "inference-node-2".to_string(),
                "inference-node-3".to_string(),
            ],
            8,
            1.5,
        ));
        let crypto_registry = Arc::new(TenantCryptoRegistry::new());
        crypto_registry.provision_tenant(sunborn_core::TenantId::new("tenant-alpha"), [0x42; 32]);
        crypto_registry.provision_tenant(
            sunborn_core::TenantId::new("enterprise-fintech"),
            [0x43; 32],
        );
        crypto_registry.provision_tenant(sunborn_core::TenantId::new("acme-corp"), [0x44; 32]);
        crypto_registry.provision_tenant(
            sunborn_core::TenantId::new("healthcare-systems"),
            [0x45; 32],
        );

        let predictive_engine = Arc::new(PredictivePrefetchEngine::default());
        let autonomic_controller = Arc::new(AutonomicSelfHealingController::new());
        let vector_clock = Arc::new(RwLock::new(VectorClock::new()));
        {
            let mut vc = vector_clock.write().unwrap();
            vc.increment("gateway-primary-node-1");
        }

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
            radix_router,
            crypto_registry,
            predictive_engine,
            autonomic_controller,
            vector_clock,
        }
    }
}
