use crate::autonomic::AutonomicSelfHealingController;
use crate::middleware::auth::ApiKeyRegistry;
use crate::origin::provider::{LlmOriginProvider, MockOriginProvider};
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
    pub dragonfly_cache: Arc<sunborn_cache::DragonflyCacheStore>,
    pub vector_store: Arc<MemoryVectorStore>,
    pub qdrant_store: Arc<sunborn_semantic::QdrantVectorStore>,
    pub decision_engine: Arc<DecisionEngine<DeterministicEmbeddingProvider, MemoryVectorStore>>,
    pub dependency_graph: Arc<DependencyGraph>,
    pub invalidation_engine: Arc<InvalidationEngine<MemoryExactCache, MemoryVectorStore>>,
    pub db: Arc<dyn sunborn_db::DatabaseStore>,
    pub memory_db: Arc<MemoryDatabaseStore>,
    pub postgres_db: Arc<sunborn_db::PostgresDatabaseStore>,
    pub execution_store: Arc<MemoryExecutionCacheStore>,
    pub shadow_evaluator: Arc<ShadowEvaluator>,
    pub radix_router: Arc<KvCacheRouter>,
    pub crypto_registry: Arc<TenantCryptoRegistry>,
    pub predictive_engine: Arc<PredictivePrefetchEngine>,
    pub autonomic_controller: Arc<AutonomicSelfHealingController>,
    pub vector_clock: Arc<RwLock<VectorClock>>,
    pub origin_provider: Arc<dyn LlmOriginProvider>,
    pub api_key_registry: Arc<ApiKeyRegistry>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let memory_cache = Arc::new(MemoryExactCache::new());
        let dragonfly_cache = Arc::new(sunborn_cache::DragonflyCacheStore::new(
            &config.cache.dragonfly_url,
        ));
        let circuit_breaker = Arc::new(CircuitBreakerCache::new(
            (*memory_cache).clone(),
            3,
            std::time::Duration::from_secs(5),
        ));
        let vector_store = Arc::new(MemoryVectorStore::new());
        let qdrant_store = Arc::new(sunborn_semantic::QdrantVectorStore::new(
            &config.cache.qdrant_url,
            "sunborn_vectors",
        ));
        let db = Arc::new(MemoryDatabaseStore::new());
        let postgres_db = Arc::new(sunborn_db::PostgresDatabaseStore::new(
            "postgres://postgres:postgres@localhost:5432/sunborn",
        ));
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

        let origin_provider: Arc<dyn LlmOriginProvider> = Arc::new(MockOriginProvider::new());
        let api_key_registry = Arc::new(crate::middleware::auth::get_global_registry().clone());

        let is_dragonfly = config.cache.backend == "dragonfly"
            || std::env::var("SUNBORN_CACHE_BACKEND")
                .map(|v| v.to_lowercase() == "dragonfly")
                .unwrap_or(false);
        let selected_cache: Arc<dyn ExactCacheStore> = if is_dragonfly {
            dragonfly_cache.clone()
        } else {
            circuit_breaker.clone()
        };

        let is_postgres = config.cache.database_backend == "postgres"
            || std::env::var("SUNBORN_DB_BACKEND")
                .map(|v| v.to_lowercase() == "postgres")
                .unwrap_or(false);
        let selected_db: Arc<dyn sunborn_db::DatabaseStore> = if is_postgres {
            postgres_db.clone()
        } else {
            db.clone()
        };

        Self {
            config,
            cache: selected_cache,
            circuit_breaker,
            memory_cache,
            dragonfly_cache,
            vector_store,
            qdrant_store,
            decision_engine,
            dependency_graph,
            invalidation_engine,
            db: selected_db,
            memory_db: db,
            postgres_db,
            execution_store,
            shadow_evaluator,
            radix_router,
            crypto_registry,
            predictive_engine,
            autonomic_controller,
            vector_clock,
            origin_provider,
            api_key_registry,
        }
    }
}
