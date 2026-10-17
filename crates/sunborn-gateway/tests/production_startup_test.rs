use sunborn_core::{AppConfig, ErrorCode};
use sunborn_gateway::AppState;
use tokio::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::const_new(());

struct EnvGuard {
    saved: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn new(keys: &[&'static str]) -> Self {
        let saved = keys.iter().map(|&k| (k, std::env::var(k).ok())).collect();
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, val) in &self.saved {
            match val {
                Some(v) => std::env::set_var(k, v),
                None => std::env::remove_var(k),
            }
        }
    }
}

const VALID_PROD_KEK: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const VALID_PROD_ADMIN_KEY: &str = "prod_admin_secret_key_entropy_99887766554433221100";

#[tokio::test]
async fn test_production_startup_rejects_default_admin_key() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", "dev-admin-key"); // placeholder

    let mut config = AppConfig::default();
    config.cache.backend = "dragonfly".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.vector_backend = "qdrant".to_string();

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(err.message.contains("SUNBORN_ADMIN_KEY"));
}

#[tokio::test]
async fn test_production_startup_rejects_mock_llm_origin() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_LLM_ORIGIN_URL",
        "OPENAI_API_BASE",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_ADMIN_KEY);
    std::env::remove_var("SUNBORN_LLM_ORIGIN_URL");
    std::env::remove_var("OPENAI_API_BASE");

    let mut config = AppConfig::default();
    config.cache.backend = "dragonfly".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.vector_backend = "qdrant".to_string();

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(err.message.contains("MockOriginProvider"));
}

#[tokio::test]
async fn test_production_startup_rejects_deterministic_embedder() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_LLM_ORIGIN_URL",
        "SUNBORN_EMBEDDING_URL",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_ADMIN_KEY);
    std::env::remove_var("SUNBORN_EMBEDDING_URL");

    let mut config = AppConfig::default();
    config.cache.backend = "dragonfly".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.vector_backend = "qdrant".to_string();

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(
        err.message.contains("DeterministicEmbeddingProvider")
            || err.message.contains("MockOriginProvider")
    );
}

#[tokio::test]
async fn test_production_startup_rejects_change_me_and_fixtures() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", "admin-secret-key-change-me");

    let mut config = AppConfig::default();
    config.cache.backend = "dragonfly".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.vector_backend = "qdrant".to_string();

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(err.message.contains("SUNBORN_ADMIN_KEY"));
}
