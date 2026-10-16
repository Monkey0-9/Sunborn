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

    let total_records = db.count_cache_entries().await.unwrap_or(0);
    info!(
        total_records,
        "Durable cache entries detected in PostgreSQL"
    );

    let batch_size = 500;
    let mut offset = 0;
    let mut exact_reconstructed = 0;
    let mut vector_reconstructed = 0;

    loop {
        let batch = db.scan_cache_entries(batch_size, offset).await?;
        if batch.is_empty() {
            break;
        }

        let batch_count = batch.len();
        offset += batch_count;

        for record in batch {
            // 1. Rebuild Exact Cache entry
            let ttl_secs = record
                .expires_at
                .map(|exp| (exp - Utc::now()).num_seconds().max(1) as u64)
                .unwrap_or(86400);

            let entry = CacheEntry::new(&record.key, &record.content, Some(ttl_secs));
            cache.set(entry).await?;
            exact_reconstructed += 1;

            // 2. Reconstruct Semantic Vector entry
            let vector = embedder.embed(&record.prompt_canonical).await?;
            let ventry = VectorEntry {
                id: record.key.clone(),
                tenant_id: TenantId::new(record.tenant_id),
                application_id: ApplicationId::new(record.application_id),
                prompt: record.prompt_canonical,
                content: record.content,
                vector,
                metadata: serde_json::json!({ "model": record.model }),
                created_at: record.created_at,
            };
            vector_store.insert(ventry).await?;
            vector_reconstructed += 1;
        }

        info!(
            scanned = offset,
            exact_reconstructed, vector_reconstructed, "Processed rebuild batch"
        );
    }

    info!(
        total_scanned = offset,
        exact_reconstructed,
        vector_reconstructed,
        "Disaster Recovery & Derived Store Rebuild Completed Successfully. 0% Data Loss."
    );

    Ok(())
}
