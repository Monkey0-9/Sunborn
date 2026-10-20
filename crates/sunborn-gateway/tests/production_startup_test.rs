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

const VALID_PROD_HMAC_SECRET: &str = "prod_learning_hmac_secret_256bit_secure_token";

#[tokio::test]
async fn test_production_startup_rejects_missing_or_insecure_learning_hmac_secret() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
        "SUNBORN_LLM_ORIGIN_URL",
        "SUNBORN_EMBEDDING_URL",
        "SUNBORN_LEARNING_HMAC_SECRET",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_ADMIN_KEY);
    std::env::set_var("SUNBORN_CACHE_BACKEND", "dragonfly");
    std::env::set_var("SUNBORN_DB_BACKEND", "postgres");
    std::env::set_var("SUNBORN_VECTOR_BACKEND", "qdrant");
    std::env::set_var("SUNBORN_LLM_ORIGIN_URL", "https://api.openai.com/v1");
    std::env::set_var("SUNBORN_EMBEDDING_URL", "https://api.openai.com/v1");

    // Case 1: Missing SUNBORN_LEARNING_HMAC_SECRET
    std::env::remove_var("SUNBORN_LEARNING_HMAC_SECRET");
    let mut config = AppConfig::default();
    config.cache.backend = "dragonfly".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.vector_backend = "qdrant".to_string();

    let state = AppState::new(config.clone());
    let res = state.verify_production_readiness().await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(err.message.contains("SUNBORN_LEARNING_HMAC_SECRET"));

    // Case 2: Insecure dev default SUNBORN_LEARNING_HMAC_SECRET
    std::env::set_var(
        "SUNBORN_LEARNING_HMAC_SECRET",
        "sunborn-insecure-dev-hmac-secret-change-in-production",
    );
    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(err.message.contains("SUNBORN_LEARNING_HMAC_SECRET"));
}

#[tokio::test]
async fn test_production_startup_rejects_key_separation_violation_learning_hmac_equals_master_kek()
{
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
        "SUNBORN_LEARNING_HMAC_SECRET",
        "SUNBORN_LLM_ORIGIN_URL",
        "SUNBORN_EMBEDDING_URL",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_ADMIN_KEY);
    // Key reuse violation: learning HMAC secret set to master KEK
    std::env::set_var("SUNBORN_LEARNING_HMAC_SECRET", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_LLM_ORIGIN_URL", "http://127.0.0.1:11434");
    std::env::set_var(
        "SUNBORN_EMBEDDING_URL",
        "http://127.0.0.1:8000/v1/embeddings",
    );

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
        err.message
            .contains("Cryptographic key separation violation")
            && err
                .message
                .contains("SUNBORN_LEARNING_HMAC_SECRET must NOT equal SUNBORN_MASTER_KEK"),
        "Expected key separation violation error, got: {}",
        err.message
    );
}

#[tokio::test]
async fn test_production_startup_rejects_key_separation_violation_admin_key_equals_master_kek() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
        "SUNBORN_LEARNING_HMAC_SECRET",
        "SUNBORN_LLM_ORIGIN_URL",
        "SUNBORN_EMBEDDING_URL",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    // Key reuse violation: admin key set to master KEK
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_LEARNING_HMAC_SECRET", VALID_PROD_HMAC_SECRET);
    std::env::set_var("SUNBORN_LLM_ORIGIN_URL", "http://127.0.0.1:11434");
    std::env::set_var(
        "SUNBORN_EMBEDDING_URL",
        "http://127.0.0.1:8000/v1/embeddings",
    );

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
        err.message
            .contains("Cryptographic key separation violation")
            && err
                .message
                .contains("SUNBORN_MASTER_KEK and SUNBORN_ADMIN_KEY must be distinct keys"),
        "Expected key separation violation error, got: {}",
        err.message
    );
}

#[tokio::test]
async fn test_production_startup_rejects_in_memory_fallbacks() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
        "SUNBORN_LEARNING_HMAC_SECRET",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_ADMIN_KEY);
    std::env::set_var("SUNBORN_LEARNING_HMAC_SECRET", VALID_PROD_HMAC_SECRET);

    // Default config has in-memory backends
    let config = AppConfig::default();
    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::ConfigurationError);
    assert!(err.message.contains("forbids ephemeral in-memory"));
}

#[tokio::test]
async fn test_configuration_matrix_adapter_selection_dev_and_prod() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
        "SUNBORN_LLM_ORIGIN_URL",
        "SUNBORN_EMBEDDING_URL",
    ]);

    std::env::remove_var("SUNBORN_ENV");
    std::env::remove_var("SUNBORN_CACHE_BACKEND");
    std::env::remove_var("SUNBORN_DB_BACKEND");
    std::env::remove_var("SUNBORN_VECTOR_BACKEND");
    std::env::remove_var("SUNBORN_LLM_ORIGIN_URL");
    std::env::remove_var("SUNBORN_EMBEDDING_URL");

    // Case 1: Default configuration selects all in-memory adapters and mock providers
    let config = AppConfig::default();
    let state = AppState::new(config);
    assert!(!state.is_dragonfly_cache_active());
    assert!(!state.is_postgres_db_active());
    assert!(!state.is_qdrant_vector_active());
    assert!(state.is_mock_origin());
    assert!(state.is_deterministic_embedder());

    // Case 2: Config explicitly selects external adapters
    let mut config2 = AppConfig::default();
    config2.cache.backend = "dragonfly".to_string();
    config2.cache.database_backend = "postgres".to_string();
    config2.cache.vector_backend = "qdrant".to_string();
    let state2 = AppState::new(config2);
    assert!(state2.is_dragonfly_cache_active());
    assert!(state2.is_postgres_db_active());
    assert!(state2.is_qdrant_vector_active());

    // Case 3: Environment variables override in-memory defaults
    std::env::set_var("SUNBORN_CACHE_BACKEND", "dragonfly");
    std::env::set_var("SUNBORN_DB_BACKEND", "postgres");
    std::env::set_var("SUNBORN_VECTOR_BACKEND", "qdrant");
    std::env::set_var("SUNBORN_LLM_ORIGIN_URL", "https://api.openai.com/v1");
    std::env::set_var("SUNBORN_EMBEDDING_URL", "https://api.openai.com/v1");

    let state3 = AppState::new(AppConfig::default());
    assert!(state3.is_dragonfly_cache_active());
    assert!(state3.is_postgres_db_active());
    assert!(state3.is_qdrant_vector_active());
    assert!(!state3.is_mock_origin());
    assert!(!state3.is_deterministic_embedder());

    // Case 4: Mixed matrix — Dragonfly active, DB memory, Vector memory
    std::env::remove_var("SUNBORN_DB_BACKEND");
    std::env::remove_var("SUNBORN_VECTOR_BACKEND");
    let state4 = AppState::new(AppConfig::default());
    assert!(state4.is_dragonfly_cache_active());
    assert!(!state4.is_postgres_db_active());
    assert!(!state4.is_qdrant_vector_active());
}

#[tokio::test]
async fn test_production_startup_failure_when_dragonfly_unavailable() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&[
        "SUNBORN_ENV",
        "SUNBORN_MASTER_KEK",
        "SUNBORN_ADMIN_KEY",
        "SUNBORN_LEARNING_HMAC_SECRET",
        "SUNBORN_CACHE_BACKEND",
        "SUNBORN_DB_BACKEND",
        "SUNBORN_VECTOR_BACKEND",
        "SUNBORN_LLM_ORIGIN_URL",
        "SUNBORN_EMBEDDING_URL",
    ]);

    std::env::set_var("SUNBORN_ENV", "production");
    std::env::set_var("SUNBORN_MASTER_KEK", VALID_PROD_KEK);
    std::env::set_var("SUNBORN_ADMIN_KEY", VALID_PROD_ADMIN_KEY);
    std::env::set_var("SUNBORN_LEARNING_HMAC_SECRET", VALID_PROD_HMAC_SECRET);
    std::env::set_var("SUNBORN_CACHE_BACKEND", "dragonfly");
    std::env::set_var("SUNBORN_DB_BACKEND", "postgres");
    std::env::set_var("SUNBORN_VECTOR_BACKEND", "qdrant");
    std::env::set_var("SUNBORN_LLM_ORIGIN_URL", "https://api.openai.com/v1");
    std::env::set_var("SUNBORN_EMBEDDING_URL", "https://api.openai.com/v1");

    let mut config = AppConfig::default();
    config.cache.backend = "dragonfly".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.vector_backend = "qdrant".to_string();
    config.cache.dragonfly_url = "redis://127.0.0.1:59999".to_string(); // unreachable port

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::CacheUnavailable);
    assert!(err.message.contains("DragonflyDB"));
}

#[tokio::test]
async fn test_production_startup_failure_when_postgres_unavailable() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&["SUNBORN_ENV", "SUNBORN_DB_BACKEND", "POSTGRES_URL"]);

    std::env::remove_var("SUNBORN_ENV");
    std::env::set_var("SUNBORN_DB_BACKEND", "postgres");
    std::env::set_var(
        "POSTGRES_URL",
        "postgres://postgres:bad@127.0.0.1:59998/sunborn",
    );

    let mut config = AppConfig::default();
    config.cache.backend = "memory".to_string();
    config.cache.database_backend = "postgres".to_string();
    config.cache.postgres_url = "postgres://postgres:bad@127.0.0.1:59998/sunborn".to_string();
    config.cache.vector_backend = "memory".to_string();

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::InternalError);
    assert!(err.message.contains("PostgreSQL"));
}

#[tokio::test]
async fn test_production_startup_failure_when_qdrant_unavailable() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&["SUNBORN_ENV", "SUNBORN_VECTOR_BACKEND"]);

    std::env::remove_var("SUNBORN_ENV");
    std::env::set_var("SUNBORN_VECTOR_BACKEND", "qdrant");

    let mut config = AppConfig::default();
    config.cache.backend = "memory".to_string();
    config.cache.database_backend = "memory".to_string();
    config.cache.vector_backend = "qdrant".to_string();
    config.cache.qdrant_url = "http://127.0.0.1:59997".to_string(); // unreachable

    let state = AppState::new(config);
    let res = state.verify_production_readiness().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::InternalError);
    assert!(err.message.contains("Qdrant"));
}

#[tokio::test]
async fn test_production_startup_failure_when_origin_unavailable() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&["SUNBORN_LLM_ORIGIN_URL"]);

    std::env::set_var("SUNBORN_LLM_ORIGIN_URL", "http://127.0.0.1:59996"); // unreachable origin

    let config = AppConfig::default();
    let state = AppState::new(config);
    let res = state.origin_provider.health_check().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::OriginError);
}

#[tokio::test]
async fn test_production_startup_failure_when_embedder_unavailable() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&["SUNBORN_EMBEDDING_URL"]);

    std::env::set_var("SUNBORN_EMBEDDING_URL", "http://127.0.0.1:59995"); // unreachable embedder

    let config = AppConfig::default();
    let state = AppState::new(config);
    let res = state.selected_embedder.health_check().await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.code, ErrorCode::OriginError);
    assert!(err.message.contains("embedding"));
}

#[tokio::test]
async fn test_service_backed_connectivity_verification_succeeds() {
    let _lock = ENV_LOCK.lock().await;
    let _guard = EnvGuard::new(&["SUNBORN_EMBEDDING_URL", "SUNBORN_LLM_ORIGIN_URL"]);

    // Mock Qdrant server responding 200 OK
    let qdrant_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let qdrant_port = qdrant_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = qdrant_listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 512];
            if let Ok(n) = socket.read(&mut buf).await {
                if n > 0 {
                    let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{\"collections\":[]}";
                    let _ = socket.write_all(resp.as_bytes()).await;
                }
            }
        }
    });

    // Mock LLM origin responding 200 OK
    let origin_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin_port = origin_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = origin_listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 512];
            if let Ok(n) = socket.read(&mut buf).await {
                if n > 0 {
                    let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 10\r\nConnection: close\r\n\r\n{\"data\":[]}";
                    let _ = socket.write_all(resp.as_bytes()).await;
                }
            }
        }
    });

    // Mock Embedder responding 200 OK
    let embedder_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let embedder_port = embedder_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = embedder_listener.accept().await {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let mut buf = [0u8; 512];
            if let Ok(n) = socket.read(&mut buf).await {
                if n > 0 {
                    let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 18\r\nConnection: close\r\n\r\n{\"data\":[{\"embedding\":[0.1]}]}";
                    let _ = socket.write_all(resp.as_bytes()).await;
                }
            }
        }
    });

    std::env::set_var(
        "SUNBORN_LLM_ORIGIN_URL",
        format!("http://127.0.0.1:{origin_port}"),
    );
    std::env::set_var(
        "SUNBORN_EMBEDDING_URL",
        format!("http://127.0.0.1:{embedder_port}"),
    );

    let mut config = AppConfig::default();
    config.cache.vector_backend = "qdrant".to_string();
    config.cache.qdrant_url = format!("http://127.0.0.1:{qdrant_port}");

    let state = AppState::new(config);

    // Verify all mock-backed services succeed actively
    assert!(state.qdrant_store.verify_connection().await.is_ok());
    assert!(state.origin_provider.health_check().await.is_ok());
    assert!(state.selected_embedder.health_check().await.is_ok());
}
