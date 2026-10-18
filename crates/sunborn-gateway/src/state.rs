use crate::autonomic::AutonomicSelfHealingController;
use crate::middleware::auth::ApiKeyRegistry;
use crate::origin::provider::{LlmOriginProvider, MockOriginProvider};
use crate::routing::predictive::PredictivePrefetchEngine;
use crate::routing::radix::KvCacheRouter;
use crate::shadow::ShadowEvaluator;
use sha2::{Digest, Sha256};
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
    pub vector_store: Arc<dyn sunborn_semantic::VectorStore>,
    pub memory_vector_store: Arc<MemoryVectorStore>,
    pub qdrant_store: Arc<sunborn_semantic::QdrantVectorStore>,
    pub decision_engine: Arc<
        DecisionEngine<
            Arc<dyn sunborn_semantic::EmbeddingProvider>,
            Arc<dyn sunborn_semantic::VectorStore>,
        >,
    >,
    pub dependency_graph: Arc<DependencyGraph>,
    pub invalidation_engine:
        Arc<InvalidationEngine<dyn ExactCacheStore, dyn sunborn_semantic::VectorStore>>,
    pub epoch_lease_manager: Arc<sunborn_invalidation::EpochLeaseManager>,
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
    pub selected_embedder: Arc<dyn sunborn_semantic::EmbeddingProvider>,
    pub learning_store: Arc<crate::learning::TenantLearningStore>,
    pub learning_controller: Arc<crate::learning::PolicyPromotionController>,
    pub learning_hmac_secret: Vec<u8>,
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
        let memory_vector_store = Arc::new(MemoryVectorStore::new());
        let qdrant_store = Arc::new(sunborn_semantic::QdrantVectorStore::new(
            &config.cache.qdrant_url,
            "sunborn_vectors",
        ));

        let is_qdrant = config.cache.vector_backend == "qdrant"
            || std::env::var("SUNBORN_VECTOR_BACKEND")
                .map(|v| v.to_lowercase() == "qdrant")
                .unwrap_or(false);
        let selected_vector: Arc<dyn sunborn_semantic::VectorStore> = if is_qdrant {
            qdrant_store.clone()
        } else {
            memory_vector_store.clone()
        };

        let db = Arc::new(MemoryDatabaseStore::new());
        let postgres_url = std::env::var("POSTGRES_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| config.cache.postgres_url.clone());
        let postgres_db = Arc::new(sunborn_db::PostgresDatabaseStore::new(&postgres_url));

        let selected_embedder: Arc<dyn sunborn_semantic::EmbeddingProvider> = if let Ok(url) =
            std::env::var("SUNBORN_EMBEDDING_URL").or_else(|_| std::env::var("EMBEDDING_API_URL"))
        {
            let api_key = std::env::var("SUNBORN_EMBEDDING_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .ok();
            let model = std::env::var("SUNBORN_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string());
            Arc::new(sunborn_semantic::HttpEmbeddingProvider::new(
                url, model, api_key,
            ))
        } else {
            Arc::new(DeterministicEmbeddingProvider::new())
        };

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

        let decision_engine = Arc::new(DecisionEngine::new(
            selected_embedder.clone(),
            selected_vector.clone(),
            DecisionConfig::default(),
        ));
        let dependency_graph = Arc::new(DependencyGraph::new());
        let invalidation_engine = Arc::new(InvalidationEngine::new(
            dependency_graph.clone(),
            selected_cache.clone(),
            selected_vector.clone(),
        ));
        let epoch_lease_manager = Arc::new(sunborn_invalidation::EpochLeaseManager::new());
        let execution_store = Arc::new(MemoryExecutionCacheStore::new());
        let shadow_evaluator = Arc::new(ShadowEvaluator::with_embedder(selected_embedder.clone()));
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
        let _ =
            crypto_registry.provision_tenant_secure(sunborn_core::TenantId::new("tenant-alpha"));
        let _ = crypto_registry
            .provision_tenant_secure(sunborn_core::TenantId::new("enterprise-fintech"));
        let _ = crypto_registry.provision_tenant_secure(sunborn_core::TenantId::new("acme-corp"));
        let _ = crypto_registry
            .provision_tenant_secure(sunborn_core::TenantId::new("healthcare-systems"));

        let predictive_engine = Arc::new(PredictivePrefetchEngine::default());
        let autonomic_controller = Arc::new(AutonomicSelfHealingController::new());
        let vector_clock = Arc::new(RwLock::new(VectorClock::new()));
        {
            let mut vc = vector_clock.write().unwrap();
            vc.increment("gateway-primary-node-1");
        }

        let origin_provider: Arc<dyn LlmOriginProvider> = if let Ok(url) =
            std::env::var("SUNBORN_LLM_ORIGIN_URL").or_else(|_| std::env::var("OPENAI_API_BASE"))
        {
            let api_key = std::env::var("SUNBORN_LLM_ORIGIN_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .ok();
            Arc::new(crate::origin::provider::HttpLlmOriginProvider::new(
                url, api_key,
            ))
        } else {
            Arc::new(MockOriginProvider::new())
        };
        let api_key_registry = Arc::new(crate::middleware::auth::get_global_registry().clone());
        let learning_store = Arc::new(crate::learning::TenantLearningStore::default());
        let learning_controller = Arc::new(crate::learning::PolicyPromotionController::default());

        let is_prod = std::env::var("SUNBORN_ENV")
            .or_else(|_| std::env::var("ENVIRONMENT"))
            .map(|v| {
                let v = v.to_lowercase();
                v == "production" || v == "prod"
            })
            .unwrap_or(false);

        let learning_hmac_secret: Vec<u8> =
            if let Ok(secret) = std::env::var("SUNBORN_LEARNING_HMAC_SECRET") {
                secret.into_bytes()
            } else if let Ok(master_kek) = std::env::var("SUNBORN_MASTER_KEK") {
                let mut hasher = Sha256::new();
                hasher.update(b"sunborn-learning-hmac-v1:");
                hasher.update(master_kek.as_bytes());
                hasher.finalize().to_vec()
            } else if is_prod {
                Vec::new()
            } else {
                b"sunborn-insecure-dev-hmac-secret-change-in-production".to_vec()
            };

        Self {
            config,
            cache: selected_cache,
            circuit_breaker,
            memory_cache,
            dragonfly_cache,
            vector_store: selected_vector,
            memory_vector_store,
            qdrant_store,
            decision_engine,
            dependency_graph,
            invalidation_engine,
            epoch_lease_manager,
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
            selected_embedder,
            learning_store,
            learning_controller,
            learning_hmac_secret,
        }
    }

    /// Verifies production infrastructure readiness and fail-closed durability contracts.
    /// In production (`SUNBORN_ENV=production`), active network connectivity to PostgreSQL,
    /// DragonflyDB, and Qdrant must succeed before the gateway can start serving traffic.
    pub async fn verify_production_readiness(&self) -> Result<(), sunborn_core::SunbornError> {
        let is_prod = std::env::var("SUNBORN_ENV")
            .or_else(|_| std::env::var("ENVIRONMENT"))
            .map(|v| {
                let v = v.to_lowercase();
                v == "production" || v == "prod"
            })
            .unwrap_or(false);

        let cache_backend = std::env::var("SUNBORN_CACHE_BACKEND")
            .unwrap_or_else(|_| self.config.cache.backend.clone());
        let db_backend = std::env::var("SUNBORN_DB_BACKEND")
            .unwrap_or_else(|_| self.config.cache.database_backend.clone());
        let vector_backend = std::env::var("SUNBORN_VECTOR_BACKEND")
            .unwrap_or_else(|_| self.config.cache.vector_backend.clone());

        // 1. Production environment strictness: disallow ephemeral in-memory stores, insecure secrets, and mock providers
        if is_prod {
            if !cache_backend.eq_ignore_ascii_case("dragonfly") {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory cache backend. Set SUNBORN_CACHE_BACKEND=dragonfly.",
                ));
            }
            if !db_backend.eq_ignore_ascii_case("postgres") {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory database backend. Set SUNBORN_DB_BACKEND=postgres.",
                ));
            }
            if !vector_backend.eq_ignore_ascii_case("qdrant") {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory vector backend. Set SUNBORN_VECTOR_BACKEND=qdrant.",
                ));
            }

            // P0-2: Fail-closed secret hygiene validation
            let master_kek = std::env::var("SUNBORN_MASTER_KEK")
                .or_else(|_| std::env::var("MASTER_KEK"))
                .map_err(|_| {
                    sunborn_core::SunbornError::configuration_error(
                        "Production requires SUNBORN_MASTER_KEK environment variable to be configured",
                    )
                })?;
            let lower_kek = master_kek.to_lowercase();
            if lower_kek.contains("changeme")
                || lower_kek.contains("placeholder")
                || lower_kek.contains("dev-master-kek")
                || lower_kek.contains("dev_insecure")
                || lower_kek.contains("dev-insecure")
                || lower_kek.contains("0123456789abcdef")
                || lower_kek.chars().all(|c| c == '0')
                || master_kek.len() < 32
            {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production SUNBORN_MASTER_KEK is using a weak or default/placeholder key. A high-entropy 256-bit key is required.",
                ));
            }

            let admin_key = std::env::var("SUNBORN_ADMIN_KEY").map_err(|_| {
                sunborn_core::SunbornError::configuration_error(
                    "Production requires SUNBORN_ADMIN_KEY environment variable to be configured",
                )
            })?;
            let lower_admin = admin_key.to_lowercase();
            if lower_admin == "dev-admin-key"
                || lower_admin == "admin"
                || lower_admin.contains("changeme")
                || lower_admin.contains("change-me")
                || lower_admin.contains("sunborn-admin-key")
                || lower_admin.contains("placeholder")
                || lower_admin.contains("dev_insecure")
                || lower_admin.contains("dev-insecure")
                || admin_key.len() < 16
            {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production SUNBORN_ADMIN_KEY is using a default or insecure value. Provide a secure, high-entropy key.",
                ));
            }

            // P0-3: Genuine LLM Origin validation
            if self.origin_provider.is_mock() {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids MockOriginProvider. Configure SUNBORN_LLM_ORIGIN_URL or OPENAI_API_BASE.",
                ));
            }

            // P0-3: Genuine Embedding Provider validation
            if self.selected_embedder.is_deterministic() {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids DeterministicEmbeddingProvider. Configure SUNBORN_EMBEDDING_URL.",
                ));
            }

            // SB-P0-04: Genuine Learning HMAC Secret validation
            let learning_hmac = std::env::var("SUNBORN_LEARNING_HMAC_SECRET").map_err(|_| {
                sunborn_core::SunbornError::configuration_error(
                    "Production requires SUNBORN_LEARNING_HMAC_SECRET environment variable to be configured",
                )
            })?;
            let lower_hmac = learning_hmac.to_lowercase();
            if lower_hmac == "sunborn-insecure-dev-hmac-secret-change-in-production"
                || lower_hmac.contains("changeme")
                || lower_hmac.contains("change-me")
                || lower_hmac.contains("insecure")
                || lower_hmac.contains("dev-")
                || lower_hmac.contains("dev_")
                || lower_hmac.contains("placeholder")
                || learning_hmac.len() < 16
            {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production SUNBORN_LEARNING_HMAC_SECRET is using a default or insecure value. Provide a secure, high-entropy key.",
                ));
            }
        }

        // 2. Validate Dragonfly cache connectivity if configured or in strict mode
        if cache_backend.eq_ignore_ascii_case("dragonfly") {
            tracing::info!(
                endpoint = %self.dragonfly_cache.connection_url,
                "Validating DragonflyDB cache connectivity for production readiness..."
            );
            self.dragonfly_cache.verify_connection().await?;
            tracing::info!("DragonflyDB production readiness verified successfully.");
        }

        // 3. Validate PostgreSQL database connectivity if configured or in strict mode
        if db_backend.eq_ignore_ascii_case("postgres") {
            tracing::info!(
                endpoint = %self.postgres_db.connection_url,
                "Validating PostgreSQL database connectivity for production readiness..."
            );
            self.postgres_db.verify_connection().await?;
            tracing::info!("PostgreSQL production readiness verified successfully.");
        }

        // 4. Validate Qdrant vector store connectivity if configured or in strict mode
        if vector_backend.eq_ignore_ascii_case("qdrant") {
            tracing::info!(
                endpoint = %self.qdrant_store.endpoint_url,
                "Validating Qdrant vector store connectivity for production readiness..."
            );
            self.qdrant_store.verify_connection().await?;
            tracing::info!("Qdrant production readiness verified successfully.");
        }

        if is_prod {
            self.origin_provider.health_check().await?;
            self.selected_embedder.health_check().await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test]
    async fn test_verify_production_readiness_rejects_memory_in_production() {
        let _lock = TEST_MUTEX.lock().await;
        let mut config = AppConfig::default();
        config.cache.backend = "memory".to_string();
        config.cache.database_backend = "memory".to_string();

        let state = AppState::new(config);
        std::env::set_var("SUNBORN_ENV", "production");
        let res = state.verify_production_readiness().await;
        std::env::remove_var("SUNBORN_ENV");

        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().code,
            sunborn_core::ErrorCode::ConfigurationError
        );
    }

    #[tokio::test]
    async fn test_verify_production_readiness_fails_on_unreachable_dragonfly() {
        let _lock = TEST_MUTEX.lock().await;
        std::env::remove_var("SUNBORN_ENV");
        let mut config = AppConfig::default();
        config.cache.backend = "dragonfly".to_string();
        config.cache.dragonfly_url = "redis://127.0.0.1:59999".to_string(); // unreachable port

        let state = AppState::new(config);
        let res = state.verify_production_readiness().await;

        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().code,
            sunborn_core::ErrorCode::CacheUnavailable
        );
    }

    #[tokio::test]
    async fn test_verify_production_readiness_rejects_placeholder_secrets() {
        let _lock = TEST_MUTEX.lock().await;
        let mut config = AppConfig::default();
        config.cache.backend = "dragonfly".to_string();
        config.cache.database_backend = "postgres".to_string();
        config.cache.vector_backend = "qdrant".to_string();

        let state = AppState::new(config);

        std::env::set_var("SUNBORN_ENV", "production");
        std::env::set_var(
            "SUNBORN_MASTER_KEK",
            "changeme_in_production_placeholder_key_too_short",
        );
        std::env::set_var("SUNBORN_ADMIN_KEY", "dev-admin-key");

        let res = state.verify_production_readiness().await;

        std::env::remove_var("SUNBORN_ENV");
        std::env::remove_var("SUNBORN_MASTER_KEK");
        std::env::remove_var("SUNBORN_ADMIN_KEY");

        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().code,
            sunborn_core::ErrorCode::ConfigurationError
        );
    }

    #[tokio::test]
    async fn test_verify_production_readiness_rejects_mock_providers_in_production() {
        let _lock = TEST_MUTEX.lock().await;
        let mut config = AppConfig::default();
        config.cache.backend = "dragonfly".to_string();
        config.cache.database_backend = "postgres".to_string();
        config.cache.vector_backend = "qdrant".to_string();

        let state = AppState::new(config);

        std::env::set_var("SUNBORN_ENV", "production");
        std::env::set_var(
            "SUNBORN_MASTER_KEK",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        );
        std::env::set_var("SUNBORN_ADMIN_KEY", "prod-secure-admin-token-998877");

        let res = state.verify_production_readiness().await;

        std::env::remove_var("SUNBORN_ENV");
        std::env::remove_var("SUNBORN_MASTER_KEK");
        std::env::remove_var("SUNBORN_ADMIN_KEY");

        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().code,
            sunborn_core::ErrorCode::ConfigurationError
        );
    }
}
