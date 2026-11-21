use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: if is_production_mode() {
                "0.0.0.0".to_string()
            } else {
                "127.0.0.1".to_string()
            },
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
    #[serde(default)]
    pub graph_dependency_cache_enabled: bool,
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
            graph_dependency_cache_enabled: true,
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

/// Canonical production environment check across all components.
/// Strictly recognizes "production" and "prod" in either `SUNBORN_ENV` or `ENVIRONMENT`.
pub fn is_production_mode() -> bool {
    let check = |key: &str| {
        std::env::var(key)
            .map(|v| {
                let s = v.trim().to_lowercase();
                s == "production" || s == "prod"
            })
            .unwrap_or(false)
    };
    check("SUNBORN_ENV") || check("ENVIRONMENT")
}

/// Determines whether test fixtures are permitted.
/// Unconditionally returns `false` in production mode.
pub fn allow_test_fixtures() -> bool {
    if is_production_mode() {
        return false;
    }
    parse_bool_env("SUNBORN_ALLOW_TEST_FIXTURES", false).unwrap_or(false)
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
            .unwrap_or_else(|_| {
                if is_production_mode() {
                    "0.0.0.0".to_string()
                } else {
                    "127.0.0.1".to_string()
                }
            });
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
        let graph_dependency_cache_enabled =
            parse_bool_env("SUNBORN_GRAPH_DEPENDENCY_CACHE_ENABLED", true)?;

        let exact_cache_enabled = if parse_bool_env("SUNBORN_DISABLE_EXACT_CACHE", false)? {
            false
        } else {
            parse_bool_env("SUNBORN_EXACT_CACHE_ENABLED", true)?
        };

        let semantic_cache_enabled = if parse_bool_env("SUNBORN_DISABLE_SEMANTIC_CACHE", false)? {
            false
        } else {
            parse_bool_env("SUNBORN_SEMANTIC_CACHE_ENABLED", true)?
        };

        let enforce_auth = if std::env::var("SUNBORN_AUTH_ENFORCE").is_ok() {
            parse_bool_env("SUNBORN_AUTH_ENFORCE", true)?
        } else {
            parse_bool_env("SUNBORN_ENFORCE_AUTH", true)?
        };

        Ok(Self {
            server: ServerConfig { host, port },
            cache: CacheConfig {
                exact_cache_enabled,
                semantic_cache_enabled,
                dragonfly_url,
                qdrant_url,
                default_ttl_secs: 3600,
                backend,
                vector_backend,
                database_backend,
                postgres_url,
                master_kek,
                strict_durability,
                graph_dependency_cache_enabled,
            },
            auth: AuthConfig {
                enforce_auth,
                allow_dev_keys: false,
            },
        })
    }

    /// Panicking loader retained for the library quick-start example and tests.
    /// Runtime code must use [`AppConfig::try_load_from_env`] so a bad
    /// configuration exits with a diagnosable error instead of a panic.
    pub fn load_from_env() -> Self {
        Self::try_load_from_env().unwrap_or_else(|err| {
            panic!("FATAL: Sunborn Gateway configuration load failed: {err}");
        })
    }

    /// Structural validation that does not touch the network. It catches the
    /// mistakes operators actually make — a zero port, a backend selected with
    /// a URL of the wrong scheme, an empty required URL — and reports every
    /// problem at once so a single run is enough to fix them all.
    pub fn validate(&self) -> crate::error::Result<()> {
        let mut problems: Vec<String> = Vec::new();

        if self.server.port == 0 {
            problems.push("server port must not be 0".to_string());
        }
        if self.server.host.trim().is_empty() {
            problems.push("server host must not be empty".to_string());
        }
        if self.server.host.contains('/') || self.server.host.contains(':') {
            problems.push(format!(
                "server host '{}' must be a bare host name or IP address, not a URL or host:port pair",
                self.server.host
            ));
        }

        if self.cache.backend.eq_ignore_ascii_case("dragonfly") {
            let url = self.cache.dragonfly_url.trim();
            if !(url.starts_with("redis://")
                || url.starts_with("rediss://")
                || url.starts_with("unix://"))
            {
                problems.push(format!(
                    "SUNBORN_CACHE_BACKEND=dragonfly requires DRAGONFLY_URL/REDIS_URL to start with redis://, rediss://, or unix:// (got '{url}')"
                ));
            }
        }

        if self.cache.vector_backend.eq_ignore_ascii_case("qdrant") {
            let url = self.cache.qdrant_url.trim();
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                problems.push(format!(
                    "SUNBORN_VECTOR_BACKEND=qdrant requires QDRANT_URL to start with http:// or https:// (got '{url}')"
                ));
            }
        }

        if self.cache.database_backend.eq_ignore_ascii_case("postgres") {
            let url = self.cache.postgres_url.trim();
            if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
                problems.push(format!(
                    "SUNBORN_DB_BACKEND=postgres requires POSTGRES_URL/DATABASE_URL to start with postgres:// or postgresql:// (got '{url}')"
                ));
            }
        }

        if is_production_mode() {
            if !self.auth.enforce_auth {
                problems.push(
                    "production mode requires authentication to be enforced (SUNBORN_AUTH_ENFORCE=true)"
                        .to_string(),
                );
            }
            match self.cache.master_kek.as_deref() {
                None | Some("") => problems
                    .push("production mode requires SUNBORN_MASTER_KEK to be set".to_string()),
                Some(_) => {}
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            Err(crate::error::SunbornError::configuration_error(format!(
                "configuration validation failed with {} problem(s):\n  - {}",
                problems.len(),
                problems.join("\n  - ")
            )))
        }
    }

    /// Effective configuration with secrets redacted. Safe to print in logs,
    /// `sunborn-gateway config-check`, and operator runbooks.
    pub fn redacted_summary(&self) -> serde_json::Value {
        serde_json::json!({
            "environment": if is_production_mode() { "production" } else { "development" },
            "server": {
                "host": self.server.host,
                "port": self.server.port,
            },
            "backends": {
                "exact_cache": self.cache.backend,
                "vector_store": self.cache.vector_backend,
                "database": self.cache.database_backend,
            },
            "cache": {
                "exact_enabled": self.cache.exact_cache_enabled,
                "semantic_enabled": self.cache.semantic_cache_enabled,
                "graph_dependency_enabled": self.cache.graph_dependency_cache_enabled,
                "strict_durability": self.cache.strict_durability,
            },
            "auth": {
                "enforce": self.auth.enforce_auth,
            },
            "secrets": {
                "master_kek": if self.cache.master_kek.as_deref().is_some_and(|k| !k.is_empty()) { "set" } else { "unset" },
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_rejects_zero_port_and_wrong_url_scheme() {
        let mut config = AppConfig::default();
        config.server.port = 0;
        config.cache.backend = "dragonfly".to_string();
        config.cache.dragonfly_url = "http://not-redis:6379".to_string();

        let err = config
            .validate()
            .expect_err("invalid config must be rejected");
        let message = err.to_string();
        assert!(message.contains("port must not be 0"), "{message}");
        assert!(
            message.contains("requires DRAGONFLY_URL/REDIS_URL"),
            "{message}"
        );
        // Every problem is reported in one pass, not just the first.
        assert!(message.contains("2 problem(s)"), "{message}");
    }

    #[test]
    fn test_validate_accepts_default_memory_profile() {
        let config = AppConfig::default();
        config
            .validate()
            .expect("in-memory development profile is valid");
    }

    #[test]
    fn test_redacted_summary_never_contains_secret_material() {
        let mut config = AppConfig::default();
        config.cache.master_kek = Some("super-secret-kek-value-that-must-not-print".to_string());

        let rendered = config.redacted_summary().to_string();
        assert!(!rendered.contains("super-secret-kek-value"));
        assert!(rendered.contains("\"master_kek\":\"set\""));
    }

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
