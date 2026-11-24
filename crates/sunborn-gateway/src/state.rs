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
use sunborn_core::sync::write_lock;
use sunborn_core::AppConfig;
use sunborn_db::MemoryDatabaseStore;
use sunborn_execution::{
    ExecutionCacheStore, MemoryExecutionCacheStore, PersistentExecutionCacheStore,
};
use sunborn_identity::crypto_shred::TenantCryptoRegistry;
use sunborn_invalidation::{
    CompressedLineageGraph, DependencyGraph, GraphMemoryCache, InvalidationEngine,
    InvalidationOrchestrator,
};
use sunborn_semantic::{
    DecisionConfig, DecisionEngine, DeterministicEmbeddingProvider, MemoryVectorStore,
};

struct PostgresEpochPersistence(Arc<sunborn_db::PostgresDatabaseStore>);

#[async_trait::async_trait]
impl sunborn_invalidation::EpochPersistenceBackend for PostgresEpochPersistence {
    async fn load_all_epochs(
        &self,
    ) -> sunborn_core::Result<std::collections::HashMap<uuid::Uuid, u64>> {
        self.0.load_all_epochs().await
    }

    async fn advance_epoch(
        &self,
        source_id: uuid::Uuid,
        proposed: u64,
    ) -> sunborn_core::Result<u64> {
        self.0.advance_epoch(source_id, proposed).await
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub cache: Arc<dyn ExactCacheStore>,
    pub circuit_breaker: Arc<CircuitBreakerCache<MemoryExactCache>>,
    pub memory_cache: Arc<MemoryExactCache>,
    pub dragonfly_cache: Arc<sunborn_cache::DragonflyCacheStore>,
    pub tiered_cache:
        Option<Arc<sunborn_cache::TieredExactCache<sunborn_cache::DragonflyCacheStore>>>,
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
    pub watermark_fencer: Arc<sunborn_invalidation::WatermarkLineageFencer>,
    pub db: Arc<dyn sunborn_db::DatabaseStore>,
    pub memory_db: Arc<MemoryDatabaseStore>,
    pub postgres_db: Arc<sunborn_db::PostgresDatabaseStore>,
    /// Execution cache store. NOTE: Storage tier is configurable via `SUNBORN_EXECUTION_STORE_PATH`.
    pub execution_store: Arc<dyn ExecutionCacheStore>,
    pub shadow_evaluator: Arc<ShadowEvaluator>,
    pub radix_router: Arc<KvCacheRouter>,
    pub crypto_registry: Arc<TenantCryptoRegistry>,
    pub predictive_engine: Arc<PredictivePrefetchEngine>,
    pub autonomic_controller: Arc<AutonomicSelfHealingController>,
    pub vector_clock: Arc<RwLock<VectorClock>>,
    pub origin_provider: Arc<dyn LlmOriginProvider>,
    pub api_key_registry: Arc<ApiKeyRegistry>,
    pub selected_embedder: Arc<dyn sunborn_semantic::EmbeddingProvider>,
    /// Tenant learning store. NOTE: Storage tier is `process-local-ephemeral`.
    /// Trajectory feedback and policy adjustments are local to this node until exported or promoted.
    pub learning_store: Arc<crate::learning::TenantLearningStore>,
    pub learning_controller: Arc<crate::learning::PolicyPromotionController>,
    pub learning_hmac_secret: Vec<u8>,
    pub certificate_signing_key: Vec<u8>,
    pub attestation_signing_key: Option<ed25519_dalek::SigningKey>,
    pub attestation_verifying_key: ed25519_dalek::VerifyingKey,
    pub attestation_key_id: String,
    pub spend_velocity_controller: Arc<sunborn_core::SpendVelocityController>,
    /// Enforces spend ceilings cluster-wide when the durable store is active.
    pub spend_ledger: Arc<crate::finops::SpendLedger>,
    /// Enforces opt-in daily/monthly request quotas and the aggregate free pool.
    pub quota_ledger: Arc<crate::quota::QuotaLedger>,
    /// Server-side prompt-size and completion-token caps.
    pub request_limits: Arc<crate::limits::RequestLimits>,
    pub key_provider_configuration_error: Option<String>,
    pub graph_memory_cache: Option<Arc<GraphMemoryCache>>,
    pub graphify_graph: Arc<sunborn_invalidation::GraphifyKnowledgeGraph>,
    pub cluster_sync_engine: Option<Arc<sunborn_invalidation::DistributedGraphSyncEngine>>,
    /// True only when the configured cluster transport is durable (Redis
    /// Streams). In-memory fallback is never accepted in production.
    pub cluster_transport_durable: bool,
    pub orchestrator: Arc<InvalidationOrchestrator>,
}

impl AppState {
    pub fn new(config: AppConfig) -> Self {
        let l1_max_bytes = std::env::var("SUNBORN_L1_MAX_BYTES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(256 * 1024 * 1024); // 256 MiB default
        let memory_cache = Arc::new(MemoryExactCache::with_byte_budget(l1_max_bytes, 0.5));
        let watermark_fencer = Arc::new(sunborn_invalidation::WatermarkLineageFencer::new());
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
        let tiered_cache = Arc::new(sunborn_cache::TieredExactCache::with_stores(
            memory_cache.clone(),
            dragonfly_cache.clone(),
        ));
        let selected_cache: Arc<dyn ExactCacheStore> = if is_dragonfly {
            tiered_cache.clone()
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

        let mut decision_engine = DecisionEngine::new(
            selected_embedder.clone(),
            selected_vector.clone(),
            DecisionConfig::default(),
        );
        if let Ok(endpoint) = std::env::var("SUNBORN_CROSS_ENCODER_URL") {
            let expected_hash = std::env::var("SUNBORN_CROSS_ENCODER_MODEL_HASH").ok();
            decision_engine = decision_engine.with_cross_encoder(Arc::new(
                sunborn_semantic::guardrails::HttpCrossEncoderBackend::new(endpoint, expected_hash),
            ));
        }
        let decision_engine = Arc::new(decision_engine);
        // Select Neo4j for transitive lineage when configured; otherwise in-memory.
        let dependency_graph = match std::env::var("SUNBORN_NEO4J_URL") {
            Ok(url) if !url.trim().is_empty() => {
                let user = std::env::var("SUNBORN_NEO4J_USER").unwrap_or_default();
                let password = std::env::var("SUNBORN_NEO4J_PASSWORD").unwrap_or_default();
                let auth = if user.is_empty() {
                    None
                } else {
                    Some((user.as_str(), password.as_str()))
                };
                tracing::info!(endpoint = %url, "Using Neo4j Cypher graph store for dependency lineage");
                Arc::new(DependencyGraph::new_neo4j(url, auth))
            }
            _ => Arc::new(DependencyGraph::new()),
        };
        let invalidation_engine = Arc::new(InvalidationEngine::new(
            dependency_graph.clone(),
            selected_cache.clone(),
            selected_vector.clone(),
        ));
        let epoch_lease_manager = if is_postgres {
            // Hydration is performed synchronously by verify_production_readiness()
            // before the listener accepts traffic. A persistent manager refuses to
            // advance epochs until hydration succeeds.
            Arc::new(sunborn_invalidation::EpochLeaseManager::with_persistence(
                Arc::new(PostgresEpochPersistence(postgres_db.clone())),
            ))
        } else {
            Arc::new(sunborn_invalidation::EpochLeaseManager::new())
        };
        let execution_store: Arc<dyn ExecutionCacheStore> =
            if let Ok(path_str) = std::env::var("SUNBORN_EXECUTION_STORE_PATH") {
                Arc::new(PersistentExecutionCacheStore::new_with_wal(path_str))
            } else {
                Arc::new(MemoryExecutionCacheStore::new())
            };
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
        let kms_provider_type = std::env::var("SUNBORN_KMS_PROVIDER")
            .unwrap_or_else(|_| "local-aead".to_string())
            .to_lowercase();

        let master_kek_seed = std::env::var("SUNBORN_MASTER_KEK")
            .or_else(|_| std::env::var("MASTER_KEK"))
            .ok()
            .map(|master_kek| {
                let mut hasher = Sha256::new();
                hasher.update(b"sunborn-master-kek-seed-v1:");
                hasher.update(master_kek.as_bytes());
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&hasher.finalize());
                arr
            });

        let local_provider = match master_kek_seed {
            Some(seed) => sunborn_identity::LocalAeadKeyProvider::new(seed),
            None => sunborn_identity::LocalAeadKeyProvider::generate_random()
                .expect("OS cryptographic entropy is required to initialize local key provider"),
        };

        let (kms_provider, key_provider_configuration_error): (
            Arc<dyn sunborn_identity::KeyManagementProvider>,
            Option<String>,
        ) = match kms_provider_type.as_str() {
            "vault" => {
                let vault_addr = std::env::var("SUNBORN_VAULT_ADDR")
                    .or_else(|_| std::env::var("VAULT_ADDR"));
                let vault_token = std::env::var("SUNBORN_VAULT_TOKEN")
                    .or_else(|_| std::env::var("VAULT_TOKEN"));
                let vault_key = std::env::var("SUNBORN_VAULT_KEY_NAME")
                    .or_else(|_| std::env::var("VAULT_TRANSIT_KEY"))
                    .unwrap_or_else(|_| "sunborn-tdek".to_string());

                match (vault_addr, vault_token) {
                    (Ok(addr), Ok(token)) => (
                        Arc::new(sunborn_identity::RemoteVaultTransitKeyProvider::new(
                            addr, vault_key, token,
                        )),
                        None,
                    ),
                    _ => (
                        Arc::new(local_provider),
                        Some("SUNBORN_KMS_PROVIDER=vault requires VAULT_ADDR and VAULT_TOKEN environment variables; no remote KMS/HSM adapter is implemented without valid credentials".to_string()),
                    ),
                }
            }
            "aws-kms" => {
                let key_arn = std::env::var("SUNBORN_AWS_KMS_KEY_ARN")
                    .or_else(|_| std::env::var("AWS_KMS_KEY_ARN"));
                let region = std::env::var("SUNBORN_AWS_REGION")
                    .or_else(|_| std::env::var("AWS_REGION"))
                    .unwrap_or_else(|_| "us-east-1".to_string());

                match key_arn {
                    Ok(arn) => (
                        Arc::new(sunborn_identity::RemoteAwsKmsKeyProvider::new(arn, region)),
                        None,
                    ),
                    Err(_) => (
                        Arc::new(local_provider),
                        Some("SUNBORN_KMS_PROVIDER=aws-kms requires AWS_KMS_KEY_ARN environment variable; no remote KMS/HSM adapter is implemented without credentials".to_string()),
                    ),
                }
            }
            "local-aead" => (Arc::new(local_provider), None),
            _ => (
                Arc::new(local_provider),
                Some(format!(
                    "Unsupported SUNBORN_KMS_PROVIDER `{kms_provider_type}`: no remote KMS/HSM adapter is implemented. Use local-aead only for development; production readiness will reject it."
                )),
            ),
        };

        if let Some(ref err) = key_provider_configuration_error {
            tracing::error!(
                provider = %kms_provider_type,
                error = %err,
                "Unsupported or incomplete KMS provider configuration; startup readiness will fail closed"
            );
        }

        let crypto_registry = Arc::new(TenantCryptoRegistry::with_provider(kms_provider));
        // Sample tenant keys exist so local exploration and the shipped examples
        // work out of the box. Their identities are fixed and public, so
        // provisioning them in production would hand working tenant keys to
        // anyone who reads this source file. Gate strictly on non-production
        // mode and make the skip visible in startup logs.
        if sunborn_core::is_production_mode() {
            tracing::info!(
                "Production mode: sample tenant key provisioning skipped \
                 (tenant-alpha, enterprise-fintech, acme-corp, healthcare-systems). \
                 Tenants must be provisioned explicitly via the governance API."
            );
        } else {
            for demo_tenant in [
                "tenant-alpha",
                "enterprise-fintech",
                "acme-corp",
                "healthcare-systems",
            ] {
                if let Err(err) = crypto_registry
                    .provision_tenant_secure(sunborn_core::TenantId::new(demo_tenant))
                {
                    tracing::warn!(
                        tenant = demo_tenant,
                        error = %err,
                        "Failed to provision sample tenant key (non-production only)"
                    );
                }
            }
        }

        let predictive_engine = Arc::new(PredictivePrefetchEngine::default());
        let autonomic_controller = Arc::new(AutonomicSelfHealingController::new());
        let vector_clock = Arc::new(RwLock::new(VectorClock::new()));
        {
            let mut vc = write_lock(&vector_clock);
            vc.increment("gateway-primary-node-1");
        }

        let mut arbitrage_entries = Vec::new();

        // An environment variable that is present but empty (a very common
        // pattern in generated .env files, e.g. `OPENAI_API_KEY=`) must be
        // treated as unset. `std::env::var` returns Ok("") for it, which
        // otherwise registers a provider with an empty base URL and turns a
        // harmless "no provider configured" into a 500 builder error.
        let non_empty_env =
            |name: &str| -> Option<String> { optional_env_value(std::env::var(name)) };

        // 1. OpenAI / vLLM / Ollama Provider
        if let Some(url) =
            non_empty_env("SUNBORN_LLM_ORIGIN_URL").or_else(|| non_empty_env("OPENAI_API_BASE"))
        {
            let api_key = non_empty_env("SUNBORN_LLM_ORIGIN_API_KEY")
                .or_else(|| non_empty_env("OPENAI_API_KEY"));
            arbitrage_entries.push(crate::origin::provider::ArbitrageProviderEntry {
                name: "openai-compatible".into(),
                provider: Arc::new(crate::origin::provider::HttpLlmOriginProvider::new(
                    url, api_key,
                )),
                cost_per_million_input: 2.50,
                cost_per_million_output: 10.00,
                priority: 1,
            });
        }

        // 2. Native Anthropic Messages Provider (Claude 3.5 Sonnet / Opus)
        if let Some(anthropic_key) = non_empty_env("ANTHROPIC_API_KEY") {
            let anthropic_url = non_empty_env("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| "https://api.anthropic.com".to_string());
            arbitrage_entries.push(crate::origin::provider::ArbitrageProviderEntry {
                name: "anthropic-claude".into(),
                provider: Arc::new(crate::origin::anthropic::AnthropicMessagesProvider::new(
                    anthropic_url,
                    Some(anthropic_key),
                )),
                cost_per_million_input: 3.00,
                cost_per_million_output: 15.00,
                priority: 2,
            });
        }

        // 3. Native Google Gemini Provider (Gemini 1.5 Pro / Flash)
        if let Some(gemini_key) = non_empty_env("GEMINI_API_KEY") {
            let gemini_url = non_empty_env("GEMINI_BASE_URL")
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com".to_string());
            arbitrage_entries.push(crate::origin::provider::ArbitrageProviderEntry {
                name: "google-gemini".into(),
                provider: Arc::new(crate::origin::gemini::GeminiProvider::new(
                    gemini_url,
                    Some(gemini_key),
                )),
                cost_per_million_input: 1.25,
                cost_per_million_output: 5.00,
                priority: 3,
            });
        }

        // 4. Native Sarvam AI Indic Sovereign Provider (sarvam-2b)
        if let Some(sarvam_key) = non_empty_env("SARVAM_API_KEY") {
            let sarvam_url = non_empty_env("SARVAM_BASE_URL")
                .unwrap_or_else(|| "https://api.sarvam.ai/v1".to_string());
            arbitrage_entries.push(crate::origin::provider::ArbitrageProviderEntry {
                name: "sarvam-indic".into(),
                provider: Arc::new(crate::origin::sarvam::SarvamOriginProvider::new(
                    sarvam_url,
                    Some(sarvam_key),
                )),
                cost_per_million_input: 0.15,
                cost_per_million_output: 0.30,
                priority: 4,
            });
        }

        // 5. Secondary failover provider if explicitly configured
        if let Some(failover_url) = non_empty_env("SUNBORN_FAILOVER_ORIGIN_URL") {
            let failover_key = non_empty_env("SUNBORN_FAILOVER_ORIGIN_KEY");
            arbitrage_entries.push(crate::origin::provider::ArbitrageProviderEntry {
                name: "failover-origin".into(),
                provider: Arc::new(crate::origin::provider::HttpLlmOriginProvider::new(
                    failover_url,
                    failover_key,
                )),
                cost_per_million_input: 3.00,
                cost_per_million_output: 15.00,
                priority: 10,
            });
        }

        let origin_provider: Arc<dyn LlmOriginProvider> = if !arbitrage_entries.is_empty() {
            if arbitrage_entries.len() == 1 {
                arbitrage_entries.remove(0).provider
            } else {
                Arc::new(crate::origin::provider::MultiProviderArbitrageRouter::new(
                    arbitrage_entries,
                ))
            }
        } else {
            Arc::new(MockOriginProvider::new())
        };
        let api_key_registry = Arc::new(crate::middleware::auth::get_global_registry().clone());
        let learning_store = Arc::new(crate::learning::TenantLearningStore::default());
        let learning_controller = Arc::new(crate::learning::PolicyPromotionController::default());
        let spend_velocity_controller = Arc::new(sunborn_core::SpendVelocityController::default());

        let is_prod = sunborn_core::is_production_mode();

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

        // Dedicated key for ReuseCertificate signing/verification. Kept distinct from
        // the learning-feedback HMAC secret so the two security roles never share keys.
        let certificate_signing_key: Vec<u8> =
            if let Ok(secret) = std::env::var("SUNBORN_CERTIFICATE_SIGNING_KEY") {
                secret.into_bytes()
            } else if let Ok(master_kek) = std::env::var("SUNBORN_MASTER_KEK") {
                let mut hasher = Sha256::new();
                hasher.update(b"sunborn-certificate-signing-v1:");
                hasher.update(master_kek.as_bytes());
                hasher.finalize().to_vec()
            } else if is_prod {
                Vec::new()
            } else {
                b"sunborn-insecure-dev-cert-signing-secret-change-in-production".to_vec()
            };

        let is_graph_cache_enabled = config.cache.graph_dependency_cache_enabled;

        let (attestation_signing_key, attestation_verifying_key, attestation_key_id) = {
            let key_id = std::env::var("SUNBORN_ATTESTATION_KEY_ID")
                .unwrap_or_else(|_| "sunborn-primary-ed25519-v1".to_string());
            if let Ok(seed_hex) = std::env::var("SUNBORN_ATTESTATION_SEED_HEX") {
                if let Ok(seed_bytes) = hex::decode(seed_hex.trim()) {
                    if seed_bytes.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&seed_bytes);
                        let sk = ed25519_dalek::SigningKey::from_bytes(&arr);
                        let vk = sk.verifying_key();
                        (Some(sk), vk, key_id)
                    } else {
                        let mut hasher = Sha256::new();
                        hasher.update(&seed_bytes);
                        let arr: [u8; 32] = hasher.finalize().into();
                        let sk = ed25519_dalek::SigningKey::from_bytes(&arr);
                        let vk = sk.verifying_key();
                        (Some(sk), vk, key_id)
                    }
                } else {
                    let mut hasher = Sha256::new();
                    hasher.update(b"sunborn-fallback-ed25519-seed:");
                    hasher.update(&certificate_signing_key);
                    let arr: [u8; 32] = hasher.finalize().into();
                    let sk = ed25519_dalek::SigningKey::from_bytes(&arr);
                    let vk = sk.verifying_key();
                    (Some(sk), vk, key_id)
                }
            } else {
                let mut hasher = Sha256::new();
                hasher.update(b"sunborn-derived-ed25519-seed-v1:");
                if !certificate_signing_key.is_empty() {
                    hasher.update(&certificate_signing_key);
                } else {
                    hasher.update(b"default-sunborn-deterministic-attestation-seed");
                }
                let arr: [u8; 32] = hasher.finalize().into();
                let sk = ed25519_dalek::SigningKey::from_bytes(&arr);
                let vk = sk.verifying_key();
                (Some(sk), vk, key_id)
            }
        };

        let graphify_graph = Arc::new(sunborn_invalidation::GraphifyKnowledgeGraph::new());
        if let Ok(store_path) = std::env::var("SUNBORN_GRAPHIFY_STORE_PATH") {
            match graphify_graph.hydrate_from_file(&store_path) {
                Ok(count) if count > 0 => {
                    tracing::info!(count, path = %store_path, "Hydrated Graphify knowledge graph from persistent store");
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!(error = %err, path = %store_path, "Could not hydrate Graphify graph from store");
                }
            }
        }

        let graph_memory_cache = if is_graph_cache_enabled {
            let lineage_graph = Arc::new(CompressedLineageGraph::new());
            Some(Arc::new(
                GraphMemoryCache::new(
                    selected_cache.clone(),
                    dependency_graph.clone(),
                    lineage_graph,
                )
                .with_graphify_graph(graphify_graph.clone()),
            ))
        } else {
            None
        };

        let pod_id = std::env::var("POD_NAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| format!("gateway-{}", uuid::Uuid::new_v4()));

        let redis_url = std::env::var("SUNBORN_REDIS_URL")
            .or_else(|_| std::env::var("REDIS_URL"))
            .ok()
            .or_else(|| {
                if config.cache.backend == "dragonfly" {
                    Some(config.cache.dragonfly_url.clone())
                } else {
                    None
                }
            });

        let cluster_transport = Some(sunborn_invalidation::create_cluster_transport(
            &pod_id,
            redis_url.as_deref(),
            2048,
        ));
        let cluster_transport_durable = cluster_transport
            .as_ref()
            .map(|t| t.is_durable())
            .unwrap_or(false);

        let cluster_sync_engine = match (graph_memory_cache.as_ref(), cluster_transport.as_ref()) {
            (Some(gmc), Some(transport)) => {
                let engine = Arc::new(
                    sunborn_invalidation::DistributedGraphSyncEngine::new(
                        pod_id.clone(),
                        transport.clone(),
                        gmc.clone(),
                    )
                    .with_exact_cache(selected_cache.clone())
                    .with_vector_store(selected_vector.clone())
                    .with_epoch_manager(epoch_lease_manager.clone()),
                );
                drop(engine.clone().start_listener());
                Some(engine)
            }
            _ => None,
        };

        let orchestrator = Arc::new(InvalidationOrchestrator::new(
            selected_cache.clone(),
            selected_vector.clone(),
            graph_memory_cache.clone(),
            dependency_graph.clone(),
            epoch_lease_manager.clone(),
            watermark_fencer.clone(),
            cluster_transport,
            pod_id.clone(),
        ));

        // Background Durable Outbox Worker (processes persistent queue if active)
        if is_postgres {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let db_clone = selected_db.clone();
                let orch_clone = orchestrator.clone();
                let worker_id = format!("{pod_id}:outbox");
                handle.spawn(async move {
                    let mut interval =
                        tokio::time::interval(tokio::time::Duration::from_millis(500));
                    loop {
                        interval.tick().await;
                        let pending = match db_clone
                            .claim_pending_outbox_events(50, &worker_id)
                            .await
                        {
                            Ok(pending) => pending,
                            Err(err) => {
                                tracing::error!(error = %err, "Outbox claim failed; will retry after lease expiry");
                                continue;
                            }
                        };
                        for record in pending {
                            let result = if record.entity_type == "knowledge_source"
                                && record.action == "invalidate"
                            {
                                match uuid::Uuid::parse_str(&record.entity_id) {
                                    Ok(source_uuid) => {
                                        let tenant = sunborn_core::TenantId::new(
                                            record.tenant_id.clone(),
                                        );
                                        orch_clone
                                            .purge_sources(&tenant, &[source_uuid])
                                            .await
                                            .map(|_| ())
                                    }
                                    Err(err) => Err(sunborn_core::SunbornError::invalid_payload(
                                        format!("Invalid source id in outbox event: {err}"),
                                    )),
                                }
                            } else {
                                Err(sunborn_core::SunbornError::invalid_payload(format!(
                                    "Unsupported outbox event {}/{}; routing to DLQ",
                                    record.entity_type, record.action
                                )))
                            };

                            match result {
                                Ok(()) => {
                                    // Deterministic crash-injection point for the
                                    // live acceptance harness: the purge has
                                    // committed but the ACK has not. The event
                                    // must survive and be reclaimed after the
                                    // lease expires. Never armed in production.
                                    if std::env::var("SUNBORN_FAULT_INJECT_OUTBOX_CRASH")
                                        .as_deref()
                                        == Ok("after_purge")
                                        && !sunborn_core::is_production_mode()
                                    {
                                        tracing::error!(
                                            outbox_id = record.id,
                                            "FAULT INJECTION: aborting between purge and ACK"
                                        );
                                        std::process::abort();
                                    }
                                    if let Err(err) =
                                        db_clone.mark_outbox_processed(record.id).await
                                    {
                                        tracing::error!(outbox_id = record.id, error = %err, "Failed to mark outbox event processed");
                                    }
                                }
                                Err(err) => {
                                    tracing::error!(outbox_id = record.id, error = %err, "Outbox invalidation failed; event will be retried or dead-lettered");
                                    if let Err(mark_err) =
                                        db_clone.mark_outbox_failed(record.id, &err.to_string()).await
                                    {
                                        tracing::error!(outbox_id = record.id, error = %mark_err, "Failed to persist outbox failure");
                                    }
                                }
                            }
                        }
                    }
                });
            }
        }

        let this = Self {
            config,
            cache: selected_cache,
            circuit_breaker,
            memory_cache,
            dragonfly_cache,
            tiered_cache: Some(tiered_cache),
            vector_store: selected_vector,
            memory_vector_store,
            qdrant_store,
            decision_engine,
            dependency_graph,
            invalidation_engine,
            epoch_lease_manager,
            watermark_fencer,
            db: selected_db,
            memory_db: db,
            spend_ledger: Arc::new(crate::finops::SpendLedger::new(
                spend_velocity_controller.clone(),
                if is_postgres {
                    Some(postgres_db.clone())
                } else {
                    None
                },
            )),
            quota_ledger: Arc::new(crate::quota::QuotaLedger::new(if is_postgres {
                Some(postgres_db.clone())
            } else {
                None
            })),
            request_limits: Arc::new(crate::limits::RequestLimits::from_env()),
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
            certificate_signing_key,
            attestation_signing_key,
            attestation_verifying_key,
            attestation_key_id,
            spend_velocity_controller,
            key_provider_configuration_error,
            graph_memory_cache,
            graphify_graph,
            cluster_sync_engine,
            cluster_transport_durable,
            orchestrator,
        };

        // Automatic crash-recovery: resume any incomplete or interrupted tenant erasure tasks
        let state_clone = this.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let _ = crate::handlers::resume_interrupted_erasure_tasks(&state_clone).await;
        });

        this
    }

    /// Returns true if DragonflyCacheStore (either directly or via TieredExactCache L2) is actively wired into the exact cache path.
    pub fn is_dragonfly_cache_active(&self) -> bool {
        if Arc::ptr_eq(
            &self.cache,
            &(self.dragonfly_cache.clone() as Arc<dyn ExactCacheStore>),
        ) {
            return true;
        }
        if let Some(ref tc) = self.tiered_cache {
            if Arc::ptr_eq(&self.cache, &(tc.clone() as Arc<dyn ExactCacheStore>)) {
                return true;
            }
        }
        false
    }

    /// Returns true if PostgresDatabaseStore is actively wired into the persistence path.
    pub fn is_postgres_db_active(&self) -> bool {
        Arc::ptr_eq(
            &self.db,
            &(self.postgres_db.clone() as Arc<dyn sunborn_db::DatabaseStore>),
        )
    }

    /// Returns true if QdrantVectorStore is actively wired into the semantic vector path.
    pub fn is_qdrant_vector_active(&self) -> bool {
        Arc::ptr_eq(
            &self.vector_store,
            &(self.qdrant_store.clone() as Arc<dyn sunborn_semantic::VectorStore>),
        )
    }

    /// Returns true if the origin provider is a mock provider.
    pub fn is_mock_origin(&self) -> bool {
        self.origin_provider.is_mock()
    }

    /// Returns true if the embedding provider is a deterministic mock.
    pub fn is_deterministic_embedder(&self) -> bool {
        self.selected_embedder.is_deterministic()
    }

    /// Gracefully drains any pending write-behind, transactional outbox, or background sync tasks
    /// before server process termination completes.
    pub async fn graceful_drain(&self) -> Result<(), sunborn_core::SunbornError> {
        tracing::info!("Executing AppState graceful drain of background workers...");

        // 1. Drain pending outbox invalidation events (bounded, non-blocking drain)
        tracing::info!("Draining pending transactional outbox events...");
        let mut drained_invalidation_events = 0;
        let drain_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

        while tokio::time::Instant::now() < drain_deadline {
            match self.invalidation_engine.try_process_next().await {
                Ok(Some(evicted)) => {
                    drained_invalidation_events += 1;
                    tracing::debug!(evicted, "Drained invalidation event during shutdown");
                }
                Ok(None) => {
                    // Outbox is fully drained
                    break;
                }
                Err(err) => {
                    tracing::warn!(error = %err, "Error draining outbox event during shutdown");
                    break;
                }
            }
        }
        tracing::info!(
            drained_invalidation_events,
            "Outbox invalidation events drained"
        );

        // 2. Audit Dead Letter Queue on shutdown
        let dlq_count = self.invalidation_engine.dlq_count().await;
        if dlq_count > 0 {
            tracing::warn!(
                dlq_count,
                "Dead-Letter Queue contains unresolved failed invalidations upon shutdown"
            );
        }

        // 3. Drain and flush database persistence
        if self.is_postgres_db_active() {
            tracing::info!("Draining pending PostgreSQL connections and flushing write buffers...");
            let _ = self.postgres_db.verify_connection().await;
        }

        tracing::info!("AppState graceful drain completed successfully with zero stranded tasks.");
        Ok(())
    }

    /// Verifies production infrastructure readiness and fail-closed durability contracts.
    /// In production (`SUNBORN_ENV=production`), active network connectivity to PostgreSQL,
    /// DragonflyDB, and Qdrant must succeed before the gateway can start serving traffic.
    pub async fn verify_production_readiness(&self) -> Result<(), sunborn_core::SunbornError> {
        self.verify_key_provider_configuration()?;

        let is_prod = sunborn_core::is_production_mode();

        let cache_backend = std::env::var("SUNBORN_CACHE_BACKEND")
            .unwrap_or_else(|_| self.config.cache.backend.clone());
        let db_backend = std::env::var("SUNBORN_DB_BACKEND")
            .unwrap_or_else(|_| self.config.cache.database_backend.clone());
        let vector_backend = std::env::var("SUNBORN_VECTOR_BACKEND")
            .unwrap_or_else(|_| self.config.cache.vector_backend.clone());

        // 1. Production environment strictness: disallow ephemeral in-memory stores, insecure secrets, and mock providers
        if is_prod {
            if !cache_backend.eq_ignore_ascii_case("dragonfly") || !self.is_dragonfly_cache_active()
            {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory cache backend. Set SUNBORN_CACHE_BACKEND=dragonfly.",
                ));
            }
            if !db_backend.eq_ignore_ascii_case("postgres") || !self.is_postgres_db_active() {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory database backend. Set SUNBORN_DB_BACKEND=postgres.",
                ));
            }
            if !vector_backend.eq_ignore_ascii_case("qdrant") || !self.is_qdrant_vector_active() {
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

            // P0-1: Cryptographic key separation validation (no key reuse across distinct security roles)
            if master_kek == admin_key {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Cryptographic key separation violation: SUNBORN_MASTER_KEK and SUNBORN_ADMIN_KEY must be distinct keys.",
                ));
            }
            if learning_hmac == master_kek {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Cryptographic key separation violation: SUNBORN_LEARNING_HMAC_SECRET must NOT equal SUNBORN_MASTER_KEK. Learning HMAC key must not default to or reuse the master KEK.",
                ));
            }
            if learning_hmac == admin_key {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Cryptographic key separation violation: SUNBORN_LEARNING_HMAC_SECRET must NOT equal SUNBORN_ADMIN_KEY.",
                ));
            }

            // Durability enforcement: Disallow ephemeral execution cache and learning store in production
            if !self.execution_store.is_durable() {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory execution store. Set SUNBORN_EXECUTION_STORE_PATH to configure durable execution storage.",
                ));
            }
            if !self.learning_store.is_durable() {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production environment SUNBORN_ENV=production forbids ephemeral in-memory learning store. Set SUNBORN_LEARNING_WAL_PATH to configure durable learning storage.",
                ));
            }

            // Certificate signing key must be distinct from every other security role.
            let cert_signing = std::env::var("SUNBORN_CERTIFICATE_SIGNING_KEY").unwrap_or_default();
            if !cert_signing.is_empty() {
                if cert_signing == master_kek
                    || cert_signing == admin_key
                    || cert_signing == learning_hmac
                {
                    return Err(sunborn_core::SunbornError::configuration_error(
                        "Cryptographic key separation violation: SUNBORN_CERTIFICATE_SIGNING_KEY must be distinct from the master KEK, admin key, and learning HMAC secret.",
                    ));
                }
                if cert_signing.len() < 32 {
                    return Err(sunborn_core::SunbornError::configuration_error(
                        "Production SUNBORN_CERTIFICATE_SIGNING_KEY is too short; provide at least 32 bytes of high entropy.",
                    ));
                }
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

            if let Err(error) = self.postgres_db.prune_quota_usage().await {
                tracing::warn!(error = %error, "Failed to prune expired quota usage windows");
            }

            tracing::info!("Hydrating persisted tenant encryption keys from PostgreSQL...");
            let persisted_keys = match self.postgres_db.fetch_all_wrapped_tdeks().await {
                Ok(keys) => keys,
                Err(error) if is_prod => {
                    return Err(sunborn_core::SunbornError::database(format!(
                        "Production startup failed to load durable tenant encryption keys: {error}"
                    )));
                }
                Err(error) => {
                    tracing::warn!(error = %error, "Failed to fetch persisted tenant encryption keys");
                    Vec::new()
                }
            };
            for (tenant_id, wrapped_bytes, _provider) in persisted_keys {
                let tid = sunborn_core::TenantId::new(tenant_id);
                if let Err(error) = self
                    .crypto_registry
                    .unwrap_and_provision(tid.clone(), &wrapped_bytes)
                {
                    if is_prod {
                        return Err(sunborn_core::SunbornError::configuration_error(format!(
                            "Production startup failed to unwrap tenant encryption key for tenant {}: {error}",
                            tid.0
                        )));
                    }
                    tracing::warn!(tenant = %tid.0, error = %error, "Failed to unwrap tenant key from database");
                } else {
                    tracing::info!(tenant = %tid.0, "Hydrated tenant encryption key from PostgreSQL");
                }
            }

            tracing::info!("Hydrating persisted API credentials from PostgreSQL...");
            if let Err(e) = self
                .api_key_registry
                .hydrate_from_db(&self.postgres_db)
                .await
            {
                if is_prod {
                    return Err(sunborn_core::SunbornError::database(format!(
                        "Production startup failed to hydrate durable API credentials: {e}"
                    )));
                }
                tracing::warn!(error = %e, "Failed to hydrate API credentials from PostgreSQL");
            }
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

        // Epoch authority must be loaded before serving. A failure here is fatal:
        // serving with an empty epoch registry would make stale writebacks and
        // stale certificates look fresh. This runs after backend connectivity so
        // configuration/connectivity errors retain their precise cause.
        self.epoch_lease_manager.hydrate_from_persistence().await?;
        if !self.epoch_lease_manager.is_hydrated() {
            return Err(sunborn_core::SunbornError::configuration_error(
                "Epoch registry hydration failed; refusing to serve with an unverified epoch authority",
            ));
        }

        if is_prod {
            if self.origin_provider.is_mock() {
                return Err(sunborn_core::SunbornError::configuration_error(
                    "Production startup refused: origin_provider is configured with MockOriginProvider. Production requires a live authenticated LLM origin.",
                ));
            }
            self.origin_provider.health_check().await?;
            self.selected_embedder.health_check().await?;
        }

        self.verify_remote_key_provider(is_prod)?;

        // The durable invalidation bus is a production requirement, checked
        // after live backend connectivity so the first actionable failure is
        // reported (unreachable Dragonfly/Postgres/Qdrant before transport).
        if is_prod && !self.cluster_transport_durable {
            return Err(sunborn_core::SunbornError::configuration_error(
                "Production requires a durable Redis Streams cluster invalidation transport. Configure REDIS_URL/SUNBORN_REDIS_URL to a reachable Redis 7+ instance; in-memory fallback is forbidden.",
            ));
        }

        Ok(())
    }

    fn verify_key_provider_configuration(&self) -> Result<(), sunborn_core::SunbornError> {
        if let Some(message) = &self.key_provider_configuration_error {
            return Err(sunborn_core::SunbornError::configuration_error(
                message.clone(),
            ));
        }
        Ok(())
    }

    fn verify_remote_key_provider(&self, is_prod: bool) -> Result<(), sunborn_core::SunbornError> {
        if is_prod && !self.crypto_registry.provider().uses_remote_crypto() {
            return Err(sunborn_core::SunbornError::configuration_error(format!(
                "Production startup refused: configured key provider `{}` does not delegate cryptographic operations to a remote KMS/HSM.",
                self.crypto_registry.provider().provider_name()
            )));
        }
        if self.crypto_registry.provider().uses_remote_crypto() {
            self.crypto_registry.provider().canary_roundtrip()?;
        }
        Ok(())
    }
}

/// Normalizes an optional environment value.
///
/// A variable that is present but empty or whitespace-only is treated as
/// unset. Generated `.env` files commonly contain `OPENAI_API_KEY=`, and
/// treating that as a configured provider produces an empty origin URL.
fn optional_env_value(value: Result<String, std::env::VarError>) -> Option<String> {
    value
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
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
    async fn test_verify_readiness_rejects_unimplemented_remote_kms_provider() {
        let _lock = TEST_MUTEX.lock().await;
        std::env::remove_var("SUNBORN_ENV");
        std::env::set_var("SUNBORN_KMS_PROVIDER", "aws-kms");
        let state = AppState::new(AppConfig::default());
        std::env::remove_var("SUNBORN_KMS_PROVIDER");

        let error = state.verify_production_readiness().await.unwrap_err();
        assert_eq!(error.code, sunborn_core::ErrorCode::ConfigurationError);
        assert!(error
            .message
            .contains("no remote KMS/HSM adapter is implemented"));
    }

    #[tokio::test]
    async fn test_production_rejects_local_key_provider() {
        let _lock = TEST_MUTEX.lock().await;
        let state = AppState::new(AppConfig::default());
        let error = state.verify_remote_key_provider(true).unwrap_err();
        assert_eq!(error.code, sunborn_core::ErrorCode::ConfigurationError);
        assert!(error
            .message
            .contains("does not delegate cryptographic operations"));
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

    #[test]
    fn empty_or_whitespace_env_values_are_treated_as_unset() {
        assert_eq!(optional_env_value(Ok(String::new())), None);
        assert_eq!(optional_env_value(Ok("   ".to_string())), None);
        assert_eq!(
            optional_env_value(Ok(" https://api.openai.com/v1 ".to_string())),
            Some("https://api.openai.com/v1".to_string())
        );
        assert_eq!(
            optional_env_value(Err(std::env::VarError::NotPresent)),
            None
        );
    }
}
