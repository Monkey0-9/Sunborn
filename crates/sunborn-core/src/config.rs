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
    pub backend: String,
    pub vector_backend: String,
    pub database_backend: String,
    pub postgres_url: String,
    pub master_kek: Option<String>,
    #[serde(default)]
    pub strict_durability: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            exact_cache_enabled: true,
            semantic_cache_enabled: true,
            dragonfly_url: "redis://127.0.0.1:6379".to_string(),
            qdrant_url: "http://127.0.0.1:6333".to_string(),
            default_ttl_secs: 3600,
            backend: "memory".to_string(),
            vector_backend: "memory".to_string(),
            database_backend: "memory".to_string(),
            postgres_url: "postgres://postgres:postgres@localhost:5432/sunborn".to_string(),
            master_kek: None,
            strict_durability: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub enforce_auth: bool,
    pub allow_dev_keys: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enforce_auth: true,
            allow_dev_keys: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub cache: CacheConfig,
    pub auth: AuthConfig,
}

/// Parses a boolean environment variable strictly.
/// Valid truthy values: "true", "1", "yes", "on".
/// Valid falsy values: "false", "0", "no", "off".
/// Any invalid string produces a fatal `SunbornError::configuration_error`.
pub fn parse_bool_env(key: &str, default: bool) -> crate::error::Result<bool> {
    match std::env::var(key) {
        Ok(val) => {
            let normalized = val.trim().to_lowercase();
            match normalized.as_str() {
                "true" | "1" | "yes" | "on" => Ok(true),
                "false" | "0" | "no" | "off" => Ok(false),
                _ => Err(crate::error::SunbornError::configuration_error(format!(
                    "Invalid boolean value for environment variable {key}: '{val}'. Expected 'true', 'false', '1', or '0'."
                ))),
            }
        }
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(crate::error::SunbornError::configuration_error(format!(
            "Failed to read environment variable {key}: {err}"
        ))),
    }
}

impl AppConfig {
    pub fn try_load_from_env() -> crate::error::Result<Self> {
        let _ = dotenvy::dotenv();
        let port = if let Ok(val) = std::env::var("PORT") {
            val.trim().parse::<u16>().map_err(|e| {
                crate::error::SunbornError::configuration_error(format!(
                    "Invalid PORT value '{val}': {e}"
                ))
            })?
        } else if let Ok(val) = std::env::var("SUNBORN_SERVER__PORT") {
            val.trim().parse::<u16>().map_err(|e| {
                crate::error::SunbornError::configuration_error(format!(
                    "Invalid SUNBORN_SERVER__PORT value '{val}': {e}"
                ))
            })?
        } else {
            8080
        };

        let host = std::env::var("HOST")
            .or_else(|_| std::env::var("SUNBORN_SERVER__HOST"))
            .unwrap_or_else(|_| "0.0.0.0".to_string());
        let dragonfly_url = std::env::var("DRAGONFLY_URL")
            .or_else(|_| std::env::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let qdrant_url = std::env::var("QDRANT_URL")
            .or_else(|_| std::env::var("SUNBORN_QDRANT_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
        let backend =
            std::env::var("SUNBORN_CACHE_BACKEND").unwrap_or_else(|_| "memory".to_string());
        let vector_backend =
            std::env::var("SUNBORN_VECTOR_BACKEND").unwrap_or_else(|_| "memory".to_string());
        let database_backend =
            std::env::var("SUNBORN_DB_BACKEND").unwrap_or_else(|_| "memory".to_string());
        let postgres_url = std::env::var("POSTGRES_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/sunborn".to_string());
        let master_kek = std::env::var("SUNBORN_MASTER_KEK")
            .or_else(|_| std::env::var("MASTER_KEK"))
            .ok();
        let strict_durability = parse_bool_env("SUNBORN_STRICT_DURABILITY", false)?;

        let enforce_auth = if std::env::var("SUNBORN_AUTH_ENFORCE").is_ok() {
            parse_bool_env("SUNBORN_AUTH_ENFORCE", true)?
        } else {
            parse_bool_env("SUNBORN_ENFORCE_AUTH", true)?
        };

        Ok(Self {
            server: ServerConfig { host, port },
            cache: CacheConfig {
                exact_cache_enabled: true,
                semantic_cache_enabled: true,
                dragonfly_url,
                qdrant_url,
                default_ttl_secs: 3600,
                backend,
                vector_backend,
                database_backend,
                postgres_url,
                master_kek,
                strict_durability,
            },
            auth: AuthConfig {
                enforce_auth,
                allow_dev_keys: false,
            },
        })
    }

    pub fn load_from_env() -> Self {
        Self::try_load_from_env().unwrap_or_else(|err| {
            panic!("FATAL: Sunborn Gateway configuration load failed: {err}");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool_env_valid() {
        std::env::set_var("TEST_SUNBORN_BOOL_TRUE", "true");
        assert!(parse_bool_env("TEST_SUNBORN_BOOL_TRUE", false).unwrap());

        std::env::set_var("TEST_SUNBORN_BOOL_ONE", "1");
        assert!(parse_bool_env("TEST_SUNBORN_BOOL_ONE", false).unwrap());

        std::env::set_var("TEST_SUNBORN_BOOL_FALSE", "false");
        assert!(!parse_bool_env("TEST_SUNBORN_BOOL_FALSE", true).unwrap());

        std::env::set_var("TEST_SUNBORN_BOOL_ZERO", "0");
        assert!(!parse_bool_env("TEST_SUNBORN_BOOL_ZERO", true).unwrap());

        assert!(parse_bool_env("TEST_SUNBORN_BOOL_NON_EXISTENT", true).unwrap());
        assert!(!parse_bool_env("TEST_SUNBORN_BOOL_NON_EXISTENT", false).unwrap());
    }

    #[test]
    fn test_parse_bool_env_invalid_fails() {
        std::env::set_var("TEST_SUNBORN_BOOL_INVALID", "invalid_string_val");
        let res = parse_bool_env("TEST_SUNBORN_BOOL_INVALID", true);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err().code,
            crate::error::ErrorCode::ConfigurationError
        );
    }
}
