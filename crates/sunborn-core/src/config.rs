use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub exact_cache_enabled: bool,
    pub semantic_cache_enabled: bool,
    pub dragonfly_url: String,
    pub qdrant_url: String,
    pub default_ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            exact_cache_enabled: true,
            semantic_cache_enabled: true,
            dragonfly_url: "redis://127.0.0.1:6379".to_string(),
            qdrant_url: "http://127.0.0.1:6334".to_string(),
            default_ttl_secs: 3600,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub cache: CacheConfig,
}

impl AppConfig {
    pub fn load_from_env() -> Self {
        let _ = dotenvy::dotenv();
        let port = std::env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8080);
        let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let dragonfly_url = std::env::var("DRAGONFLY_URL")
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let qdrant_url = std::env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());

        Self {
            server: ServerConfig { host, port },
            cache: CacheConfig {
                exact_cache_enabled: true,
                semantic_cache_enabled: true,
                dragonfly_url,
                qdrant_url,
                default_ttl_secs: 3600,
            },
        }
    }
}
