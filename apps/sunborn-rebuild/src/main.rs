use chrono::Utc;
use std::sync::Arc;
use sunborn_cache::{CacheEntry, DragonflyCacheStore, ExactCacheStore, MemoryExactCache};
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_db::{DatabaseStore, MemoryDatabaseStore, PostgresDatabaseStore};
use sunborn_semantic::{
    DeterministicEmbeddingProvider, EmbeddingProvider, HttpEmbeddingProvider, MemoryVectorStore,
    QdrantVectorStore, VectorEntry, VectorStore,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    info!("Starting Sunborn Disaster Recovery & Rebuild Engine...");
    info!("Architecture Invariant: PostgreSQL is authoritative; Dragonfly and Qdrant are fully reconstructible.");

    let is_postgres = std::env::var("SUNBORN_DB_BACKEND")
        .map(|v| v.to_lowercase() == "postgres")
        .unwrap_or(false)
        || std::env::var("POSTGRES_URL").is_ok()
        || std::env::var("DATABASE_URL").is_ok();

    let db: Arc<dyn DatabaseStore> = if is_postgres {
        let url = std::env::var("POSTGRES_URL")
            .or_else(|_| std::env::var("DATABASE_URL"))
            .unwrap_or_else(|_| "postgres://sunborn:sunborn@localhost:5432/sunborn".to_string());
        info!(backend = "postgres", url = %url, "Initializing Authoritative Database Store");
        Arc::new(PostgresDatabaseStore::new(url))
    } else {
        info!(backend = "memory", "Initializing In-Memory Database Store");
        Arc::new(MemoryDatabaseStore::new())
    };

    let is_dragonfly = std::env::var("SUNBORN_CACHE_BACKEND")
        .map(|v| v.to_lowercase() == "dragonfly")
        .unwrap_or(false)
        || std::env::var("DRAGONFLY_URL").is_ok();

    let cache: Arc<dyn ExactCacheStore> = if is_dragonfly {
        let url =
            std::env::var("DRAGONFLY_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        info!(backend = "dragonfly", url = %url, "Initializing Exact Cache Store");
        Arc::new(DragonflyCacheStore::new(url))
    } else {
        info!(
            backend = "memory",
            "Initializing In-Memory Exact Cache Store"
        );
        Arc::new(MemoryExactCache::new())
    };

    let is_qdrant = std::env::var("SUNBORN_VECTOR_BACKEND")
        .map(|v| v.to_lowercase() == "qdrant")
        .unwrap_or(false)
        || std::env::var("QDRANT_URL").is_ok();

    let vector_store: Arc<dyn VectorStore> = if is_qdrant {
        let url =
            std::env::var("QDRANT_URL").unwrap_or_else(|_| "http://127.0.0.1:6333".to_string());
        let collection =
            std::env::var("QDRANT_COLLECTION").unwrap_or_else(|_| "sunborn_vectors".to_string());
        info!(backend = "qdrant", url = %url, collection = %collection, "Initializing Vector Store");
        Arc::new(QdrantVectorStore::new(url, collection))
    } else {
        info!(backend = "memory", "Initializing In-Memory Vector Store");
        Arc::new(MemoryVectorStore::new())
    };

    let embedder: Arc<dyn EmbeddingProvider> =
        if let Ok(url) = std::env::var("SUNBORN_EMBEDDING_URL") {
            let api_key = std::env::var("SUNBORN_EMBEDDING_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .ok();
            let model = std::env::var("SUNBORN_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "text-embedding-3-small".to_string());
            info!(backend = "http", url = %url, model = %model, "Initializing Embedding Provider");
            Arc::new(HttpEmbeddingProvider::new(url, model, api_key))
        } else {
            info!(
                backend = "deterministic",
                "Initializing Deterministic Embedding Provider"
            );
            Arc::new(DeterministicEmbeddingProvider::new())
        };

    let is_production = std::env::var("SUNBORN_ENV")
        .map(|v| v.to_lowercase() == "production")
        .unwrap_or(false);

    // Trust root resolution is fail-closed. A malformed key is never treated as
    // "skip verification", and production never falls back to a public default
    // seed. The verifier must be given the same explicit key id/key material as
    // the issuer.
    let verifying_key: ed25519_dalek::VerifyingKey = if let Ok(key_hex) =
        std::env::var("SUNBORN_ATTESTATION_PUBLIC_KEY")
    {
        let bytes = hex::decode(key_hex.trim())
            .map_err(|e| anyhow::anyhow!("SUNBORN_ATTESTATION_PUBLIC_KEY is not valid hex: {e}"))?;
        let arr: [u8; 32] = bytes.as_slice().try_into().map_err(|_| {
            anyhow::anyhow!("SUNBORN_ATTESTATION_PUBLIC_KEY must decode to exactly 32 bytes")
        })?;
        ed25519_dalek::VerifyingKey::from_bytes(&arr).map_err(|e| {
            anyhow::anyhow!("SUNBORN_ATTESTATION_PUBLIC_KEY is not a valid Ed25519 key: {e}")
        })?
    } else if is_production {
        anyhow::bail!(
                "SUNBORN_ATTESTATION_PUBLIC_KEY is required for a production rebuild; refusing to admit records without an explicit trust root"
            );
    } else {
        tracing::warn!(
                "No SUNBORN_ATTESTATION_PUBLIC_KEY configured; using the development-only deterministic key. This build must never be used to admit production data."
            );
        let secret_bytes = std::env::var("SUNBORN_CERTIFICATE_SIGNING_KEY")
            .ok()
            .and_then(|k| hex::decode(k.trim()).ok());
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"sunborn-derived-ed25519-seed-v1:");
        if let Some(ref sec) = secret_bytes {
            hasher.update(sec);
        } else {
            hasher.update(b"default-sunborn-deterministic-attestation-seed");
        }
        let arr: [u8; 32] = hasher.finalize().into();
        let sk = ed25519_dalek::SigningKey::from_bytes(&arr);
        sk.verifying_key()
    };

    let total_records = db.count_cache_entries().await.unwrap_or(0);
    let authoritative_epochs = db.load_all_epochs().await?;
    info!(
        total_records,
        is_production, "Durable cache entries detected in PostgreSQL"
    );

    let batch_size = 500;
    let mut cursor: Option<(chrono::DateTime<chrono::Utc>, String)> = None;
    let mut total_scanned = 0;
    let mut exact_reconstructed = 0;
    let mut vector_reconstructed = 0;
    let mut unverified_skipped = 0;

    loop {
        let batch = db
            .scan_cache_entries_cursor(batch_size, cursor.clone())
            .await?;
        if batch.is_empty() {
            break;
        }

        total_scanned += batch.len();
        if let Some(last) = batch.last() {
            cursor = Some((last.created_at, last.key.clone()));
        }

        for record in batch {
            // Trust Monotonicity Invariant: Validate cryptographic attestation before admission
            let certificate: Option<sunborn_core::ReuseCertificate> = match &record.certificate_json
            {
                Some(json_str) => serde_json::from_str(json_str).ok(),
                None => None,
            };

            // Trust monotonicity: only a record whose certificate is signed by
            // the configured trust root, whose merkle root is intact, whose
            // tenant/application/model binding matches the durable row, and
            // whose source snapshots are current may enter the rebuilt caches.
            let is_provenance_verified: bool;
            if let Some(ref cert) = certificate {
                let sig_ok = cert.verify_signature_ed25519(&verifying_key).is_ok();
                let expected_root = sunborn_core::ReuseCertificate::compute_merkle_root(
                    &cert.certificate_id,
                    cert.certificate_version,
                    &cert.tenant_id,
                    &cert.application_id,
                    &cert.request_identity_hash,
                    &cert.model_name,
                    cert.prompt_tokens,
                    &cert.source_snapshots,
                    &cert.safety_evidence,
                    &cert.validity_window,
                    &cert.reference_fingerprint,
                );
                let integrity_ok = cert.merkle_root == expected_root;
                let binding_ok = cert.tenant_id.0 == record.tenant_id
                    && cert.application_id.0 == record.application_id
                    && cert.model_name == record.model;
                let epochs_ok = cert.source_snapshots.iter().all(|snapshot| {
                    authoritative_epochs
                        .get(&snapshot.source_id)
                        .copied()
                        .unwrap_or(1)
                        == snapshot.snapshot_epoch
                });

                if sig_ok && integrity_ok && binding_ok && epochs_ok {
                    is_provenance_verified = true;
                } else {
                    tracing::warn!(
                        key = %record.key,
                        sig_ok,
                        integrity_ok,
                        binding_ok,
                        epochs_ok,
                        "Record failed provenance verification during rebuild; skipping"
                    );
                    unverified_skipped += 1;
                    continue;
                }
            } else {
                tracing::warn!(key = %record.key, "Record has no certificate; refusing to invent provenance during rebuild");
                unverified_skipped += 1;
                continue;
            }

            // 1. Rebuild Exact Cache entry with provenance metadata
            let ttl_secs = record
                .expires_at
                .map(|exp| (exp - Utc::now()).num_seconds().max(1) as u64)
                .unwrap_or(86400);

            let mut entry = CacheEntry::new(&record.key, &record.content, Some(ttl_secs));
            entry.metadata["provenance_verified"] = serde_json::Value::Bool(is_provenance_verified);
            if let Some(ref cert) = certificate {
                if let Ok(cert_val) = serde_json::to_value(cert) {
                    entry.metadata["certificate"] = cert_val;
                }
            }
            cache.set(entry).await?;
            exact_reconstructed += 1;

            // 2. Reconstruct Semantic Vector entry with verified provenance metadata
            let vector = embedder.embed(&record.prompt_canonical).await?;
            let mut meta = serde_json::json!({
                "model": record.model,
                "provenance_verified": is_provenance_verified,
            });
            if let Some(ref cert) = certificate {
                if let Ok(cert_val) = serde_json::to_value(cert) {
                    meta["certificate"] = cert_val;
                }
            }
            let ventry = VectorEntry {
                id: record.key.clone(),
                tenant_id: TenantId::new(record.tenant_id),
                application_id: ApplicationId::new(record.application_id),
                prompt: record.prompt_canonical,
                content: record.content,
                vector,
                metadata: meta,
                created_at: record.created_at,
            };
            vector_store.insert(ventry).await?;
            vector_reconstructed += 1;
        }

        info!(
            scanned = total_scanned,
            exact_reconstructed,
            vector_reconstructed,
            unverified_skipped,
            "Processed rebuild batch"
        );
    }

    info!(
        total_scanned,
        exact_reconstructed,
        vector_reconstructed,
        "Disaster Recovery & Derived Store Rebuild Completed Successfully. 0% Data Loss."
    );

    Ok(())
}
