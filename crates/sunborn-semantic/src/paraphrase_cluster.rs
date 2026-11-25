use crate::embedding::{cosine_similarity, Vector};
use crate::guardrails::StructuralFingerprint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use sunborn_core::sync::{read_lock, write_lock};
use sunborn_core::{ApplicationId, Result, SunbornError, TenantId};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterMember {
    pub cache_key: String,
    pub prompt: String,
    pub vector: Vector,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaphraseCluster {
    pub cluster_id: String,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub representative_id: String,
    pub representative_prompt: String,
    pub centroid: Vector,
    pub structural_signature: String,
    pub members: HashMap<String, ClusterMember>,
    pub version: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub invalidated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedClusters {
    clusters: HashMap<String, ParaphraseCluster>,
    member_index: HashMap<String, String>,
}

/// Persistent paraphrase clustering manager.
/// Maintains semantic paraphrase clusters, assigns equivalent queries to cluster identities,
/// supports merging/splitting/recalibration of cluster representatives, and cascades cluster-wide
/// invalidations when meaning or dependencies change.
pub struct ParaphraseClusterManager {
    clusters: RwLock<HashMap<String, ParaphraseCluster>>,
    member_to_cluster: RwLock<HashMap<String, String>>,
    persist_path: Option<PathBuf>,
}

impl Default for ParaphraseClusterManager {
    fn default() -> Self {
        let default_path = std::env::var("SUNBORN_PARAPHRASE_CLUSTER_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        Self::new_with_persistence(default_path)
    }
}

impl ParaphraseClusterManager {
    pub fn new() -> Self {
        Self::new_with_persistence(None)
    }

    pub fn new_with_persistence(persist_path: Option<PathBuf>) -> Self {
        let mut initial_clusters = HashMap::new();
        let mut initial_index = HashMap::new();

        if let Some(ref path) = persist_path {
            if path.exists() {
                if let Ok(file) = File::open(path) {
                    if let Ok(state) = serde_json::from_reader::<_, PersistedClusters>(file) {
                        initial_clusters = state.clusters;
                        initial_index = state.member_index;
                    }
                }
            }
        }

        Self {
            clusters: RwLock::new(initial_clusters),
            member_to_cluster: RwLock::new(initial_index),
            persist_path,
        }
    }

    pub fn is_durable(&self) -> bool {
        self.persist_path.is_some()
    }

    fn persist_locked(
        persist_path: Option<&Path>,
        clusters: &HashMap<String, ParaphraseCluster>,
        member_index: &HashMap<String, String>,
    ) {
        let Some(path) = persist_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = path.with_extension(format!("tmp.{}", Uuid::new_v4()));
        let state = PersistedClusters {
            clusters: clusters.clone(),
            member_index: member_index.clone(),
        };
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&tmp_path)
        {
            if let Ok(bytes) = serde_json::to_vec(&state) {
                if file.write_all(&bytes).is_ok() && file.sync_all().is_ok() {
                    drop(file);
                    #[cfg(windows)]
                    if path.exists() {
                        let _ = std::fs::remove_file(path);
                    }
                    if std::fs::rename(&tmp_path, path).is_ok() {
                        return;
                    }
                }
            }
        }
        let _ = std::fs::remove_file(&tmp_path);
    }

    fn normalize_vector(mut v: Vector) -> Vector {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for val in &mut v {
                *val /= norm;
            }
        }
        v
    }

    fn recompute_centroid_and_medoid(cluster: &mut ParaphraseCluster) {
        if cluster.members.is_empty() {
            return;
        }
        let dim = cluster
            .members
            .values()
            .next()
            .map(|m| m.vector.len())
            .unwrap_or(0);
        if dim == 0 {
            return;
        }

        let mut sum = vec![0.0_f32; dim];
        for member in cluster.members.values() {
            for (i, &val) in member.vector.iter().enumerate().take(dim) {
                sum[i] += val;
            }
        }
        let centroid = Self::normalize_vector(sum);

        let mut best_key = cluster.representative_id.clone();
        let mut best_prompt = cluster.representative_prompt.clone();
        let mut best_sim = f32::NEG_INFINITY;

        for (key, member) in &cluster.members {
            let sim = cosine_similarity(&member.vector, &centroid);
            if sim > best_sim || (sim == best_sim && key < &best_key) {
                best_sim = sim;
                best_key = key.clone();
                best_prompt = member.prompt.clone();
            }
        }

        cluster.centroid = centroid;
        cluster.representative_id = best_key;
        cluster.representative_prompt = best_prompt;
        cluster.version += 1;
        cluster.updated_at = Utc::now();
    }

    /// Assigns a query to an existing compatible paraphrase cluster or creates a new one.
    /// Returns `(cluster_id, created_new)`.
    pub fn assign_or_create(
        &self,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        cache_key: &str,
        prompt: &str,
        vector: &[f32],
        assign_threshold: f32,
    ) -> (String, bool) {
        let fp = StructuralFingerprint::extract(prompt);
        let now = Utc::now();

        let mut clusters = write_lock(&self.clusters);
        let mut index = write_lock(&self.member_to_cluster);

        // If this cache_key is already assigned to an active cluster, update/keep membership
        if let Some(existing_cid) = index.get(cache_key).cloned() {
            if let Some(existing) = clusters.get_mut(&existing_cid) {
                if !existing.invalidated
                    && existing.tenant_id == *tenant_id
                    && existing.application_id == *application_id
                {
                    existing.members.insert(
                        cache_key.to_string(),
                        ClusterMember {
                            cache_key: cache_key.to_string(),
                            prompt: prompt.to_string(),
                            vector: Self::normalize_vector(vector.to_vec()),
                            added_at: now,
                        },
                    );
                    Self::recompute_centroid_and_medoid(existing);
                    Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
                    return (existing_cid, false);
                }
            }
        }

        // Find highest-similarity active, structurally compatible cluster in same tenant/app
        let mut best_cluster_id: Option<String> = None;
        let mut best_sim = assign_threshold;

        for (cid, cluster) in clusters.iter() {
            if cluster.invalidated
                || cluster.tenant_id != *tenant_id
                || cluster.application_id != *application_id
            {
                continue;
            }
            let rep_fp = StructuralFingerprint::extract(&cluster.representative_prompt);
            if !fp.matches(&rep_fp) {
                continue;
            }
            let sim = cosine_similarity(vector, &cluster.centroid);
            if sim >= best_sim {
                best_sim = sim;
                best_cluster_id = Some(cid.clone());
            }
        }

        if let Some(cid) = best_cluster_id {
            if let Some(cluster) = clusters.get_mut(&cid) {
                cluster.members.insert(
                    cache_key.to_string(),
                    ClusterMember {
                        cache_key: cache_key.to_string(),
                        prompt: prompt.to_string(),
                        vector: Self::normalize_vector(vector.to_vec()),
                        added_at: now,
                    },
                );
                Self::recompute_centroid_and_medoid(cluster);
                index.insert(cache_key.to_string(), cid.clone());
                Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
                return (cid, false);
            }
        }

        // Create new persistent paraphrase cluster
        let cluster_id = format!("pcluster-{}", Uuid::new_v4());
        let mut members = HashMap::new();
        let norm_vec = Self::normalize_vector(vector.to_vec());
        members.insert(
            cache_key.to_string(),
            ClusterMember {
                cache_key: cache_key.to_string(),
                prompt: prompt.to_string(),
                vector: norm_vec.clone(),
                added_at: now,
            },
        );
        let new_cluster = ParaphraseCluster {
            cluster_id: cluster_id.clone(),
            tenant_id: tenant_id.clone(),
            application_id: application_id.clone(),
            representative_id: cache_key.to_string(),
            representative_prompt: prompt.to_string(),
            centroid: norm_vec,
            structural_signature: fp.fingerprint_hash,
            members,
            version: 1,
            created_at: now,
            updated_at: now,
            invalidated: false,
        };
        clusters.insert(cluster_id.clone(), new_cluster);
        index.insert(cache_key.to_string(), cluster_id.clone());
        Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        (cluster_id, true)
    }

    pub fn get_cluster(&self, cluster_id: &str) -> Option<ParaphraseCluster> {
        read_lock(&self.clusters).get(cluster_id).cloned()
    }

    pub fn cluster_for_member(&self, cache_key: &str) -> Option<String> {
        read_lock(&self.member_to_cluster).get(cache_key).cloned()
    }

    pub fn list_tenant_clusters(&self, tenant_id: &TenantId) -> Vec<ParaphraseCluster> {
        read_lock(&self.clusters)
            .values()
            .filter(|c| &c.tenant_id == tenant_id && !c.invalidated)
            .cloned()
            .collect()
    }

    /// Recalibrates centroid and medoid representative for an existing cluster.
    pub fn recalibrate_representative(&self, cluster_id: &str) -> Result<ParaphraseCluster> {
        let mut clusters = write_lock(&self.clusters);
        let index = read_lock(&self.member_to_cluster);
        let cluster = clusters.get_mut(cluster_id).ok_or_else(|| {
            SunbornError::validation_error(format!("Paraphrase cluster '{}' not found", cluster_id))
        })?;
        Self::recompute_centroid_and_medoid(cluster);
        let updated = cluster.clone();
        Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        Ok(updated)
    }

    /// Merges two structurally compatible paraphrase clusters within the same tenant/application.
    pub fn merge_clusters(
        &self,
        target_cluster_id: &str,
        source_cluster_id: &str,
        min_merge_similarity: f32,
    ) -> Result<ParaphraseCluster> {
        if target_cluster_id == source_cluster_id {
            return Err(SunbornError::validation_error(
                "Cannot merge a paraphrase cluster with itself",
            ));
        }

        let mut clusters = write_lock(&self.clusters);
        let mut index = write_lock(&self.member_to_cluster);

        let target = clusters
            .get(target_cluster_id)
            .ok_or_else(|| {
                SunbornError::validation_error(format!(
                    "Target cluster '{}' not found",
                    target_cluster_id
                ))
            })?
            .clone();
        let source = clusters
            .get(source_cluster_id)
            .ok_or_else(|| {
                SunbornError::validation_error(format!(
                    "Source cluster '{}' not found",
                    source_cluster_id
                ))
            })?
            .clone();

        if target.invalidated || source.invalidated {
            return Err(SunbornError::validation_error(
                "Cannot merge invalidated paraphrase clusters",
            ));
        }
        if target.tenant_id != source.tenant_id || target.application_id != source.application_id {
            return Err(SunbornError::forbidden(
                "Cross-tenant or cross-application cluster merge forbidden",
            ));
        }

        let fp_t = StructuralFingerprint::extract(&target.representative_prompt);
        let fp_s = StructuralFingerprint::extract(&source.representative_prompt);
        if !fp_t.matches(&fp_s) {
            return Err(SunbornError::validation_error(
                "Cannot merge clusters with incompatible structural fingerprints",
            ));
        }

        let sim = cosine_similarity(&target.centroid, &source.centroid);
        if sim < min_merge_similarity {
            return Err(SunbornError::validation_error(format!(
                "Cluster centroid similarity {:.3} is below merge threshold {:.3}",
                sim, min_merge_similarity
            )));
        }

        let mut merged = target;
        for (k, v) in source.members {
            index.insert(k.clone(), merged.cluster_id.clone());
            merged.members.insert(k, v);
        }
        Self::recompute_centroid_and_medoid(&mut merged);

        clusters.remove(source_cluster_id);
        clusters.insert(merged.cluster_id.clone(), merged.clone());
        Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        Ok(merged)
    }

    /// Splits a cluster when members diverge below `cohesion_threshold` from the centroid.
    /// Returns the resulting cohesive clusters (1 cluster if already cohesive, or 2+ split clusters).
    pub fn split_cluster(
        &self,
        cluster_id: &str,
        cohesion_threshold: f32,
    ) -> Result<Vec<ParaphraseCluster>> {
        let mut clusters = write_lock(&self.clusters);
        let mut index = write_lock(&self.member_to_cluster);

        let cluster = clusters
            .get(cluster_id)
            .ok_or_else(|| {
                SunbornError::validation_error(format!(
                    "Paraphrase cluster '{}' not found",
                    cluster_id
                ))
            })?
            .clone();

        if cluster.members.len() < 2 {
            return Ok(vec![cluster]);
        }

        let has_outlier = cluster
            .members
            .values()
            .any(|m| cosine_similarity(&m.vector, &cluster.centroid) < cohesion_threshold);

        if !has_outlier {
            return Ok(vec![cluster]);
        }

        // Partition via farthest-pair seeding into two maximally separated sub-clusters
        let members: Vec<ClusterMember> = cluster.members.values().cloned().collect();
        let mut seed_a_idx = 0usize;
        let mut seed_b_idx = 1usize;
        let mut min_pair_sim = f32::INFINITY;

        for i in 0..members.len() {
            for j in (i + 1)..members.len() {
                let s = cosine_similarity(&members[i].vector, &members[j].vector);
                if s < min_pair_sim {
                    min_pair_sim = s;
                    seed_a_idx = i;
                    seed_b_idx = j;
                }
            }
        }

        let seed_a_vec = members[seed_a_idx].vector.clone();
        let seed_b_vec = members[seed_b_idx].vector.clone();

        let mut group_a = HashMap::new();
        let mut group_b = HashMap::new();

        for m in members {
            let sim_a = cosine_similarity(&m.vector, &seed_a_vec);
            let sim_b = cosine_similarity(&m.vector, &seed_b_vec);
            if sim_a >= sim_b {
                group_a.insert(m.cache_key.clone(), m);
            } else {
                group_b.insert(m.cache_key.clone(), m);
            }
        }

        if group_a.is_empty() || group_b.is_empty() {
            return Ok(vec![cluster]);
        }

        clusters.remove(cluster_id);
        let now = Utc::now();
        let mut result = Vec::with_capacity(2);

        for (idx_group, group) in [group_a, group_b].into_iter().enumerate() {
            let new_cid = if idx_group == 0 {
                cluster_id.to_string()
            } else {
                format!("pcluster-{}", Uuid::new_v4())
            };
            let first = group.values().next().unwrap().clone();
            let mut sub = ParaphraseCluster {
                cluster_id: new_cid.clone(),
                tenant_id: cluster.tenant_id.clone(),
                application_id: cluster.application_id.clone(),
                representative_id: first.cache_key.clone(),
                representative_prompt: first.prompt.clone(),
                centroid: first.vector.clone(),
                structural_signature: StructuralFingerprint::extract(&first.prompt)
                    .fingerprint_hash,
                members: group,
                version: cluster.version + 1,
                created_at: cluster.created_at,
                updated_at: now,
                invalidated: false,
            };
            for key in sub.members.keys() {
                index.insert(key.clone(), new_cid.clone());
            }
            Self::recompute_centroid_and_medoid(&mut sub);
            clusters.insert(new_cid, sub.clone());
            result.push(sub);
        }

        Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        Ok(result)
    }

    /// Invalidates an entire paraphrase cluster and returns all member cache keys for cascade eviction.
    pub fn invalidate_cluster(&self, cluster_id: &str) -> Vec<String> {
        let mut clusters = write_lock(&self.clusters);
        let mut index = write_lock(&self.member_to_cluster);

        let keys = if let Some(mut removed) = clusters.remove(cluster_id) {
            removed.invalidated = true;
            let member_keys: Vec<String> = removed.members.keys().cloned().collect();
            for k in &member_keys {
                index.remove(k);
            }
            member_keys
        } else {
            Vec::new()
        };

        if !keys.is_empty() {
            Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        }
        keys
    }

    /// Invalidates the entire paraphrase cluster owning `cache_key` and returns all sibling cache keys.
    pub fn invalidate_by_member(&self, cache_key: &str) -> Vec<String> {
        let cid_opt = read_lock(&self.member_to_cluster).get(cache_key).cloned();
        if let Some(cid) = cid_opt {
            self.invalidate_cluster(&cid)
        } else {
            Vec::new()
        }
    }

    /// Removes a single member from its cluster and recalibrates the remaining representative.
    pub fn remove_member_and_recalibrate(&self, cache_key: &str) -> Option<String> {
        let mut clusters = write_lock(&self.clusters);
        let mut index = write_lock(&self.member_to_cluster);

        let cid = index.remove(cache_key)?;
        let should_remove_cluster = if let Some(cluster) = clusters.get_mut(&cid) {
            cluster.members.remove(cache_key);
            if cluster.members.is_empty() {
                true
            } else {
                Self::recompute_centroid_and_medoid(cluster);
                false
            }
        } else {
            false
        };

        if should_remove_cluster {
            clusters.remove(&cid);
        }
        Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        Some(cid)
    }

    /// GDPR / Tenant erasure: purges all paraphrase clusters and membership records for a tenant.
    pub fn purge_tenant(&self, tenant_id: &TenantId) -> usize {
        let mut clusters = write_lock(&self.clusters);
        let mut index = write_lock(&self.member_to_cluster);

        let target_cids: Vec<String> = clusters
            .iter()
            .filter(|(_, c)| &c.tenant_id == tenant_id)
            .map(|(cid, _)| cid.clone())
            .collect();

        let removed_count = target_cids.len();
        for cid in target_cids {
            if let Some(c) = clusters.remove(&cid) {
                for key in c.members.keys() {
                    index.remove(key);
                }
            }
        }

        if removed_count > 0 {
            Self::persist_locked(self.persist_path.as_deref(), &clusters, &index);
        }
        removed_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{DeterministicEmbeddingProvider, EmbeddingProvider};

    #[tokio::test]
    async fn test_paraphrase_cluster_lifecycle_merge_split_recalibrate_and_persist() {
        let tmp_dir =
            std::env::temp_dir().join(format!("sunborn-pcluster-test-{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&tmp_dir);
        let store_path = tmp_dir.join("clusters.json");

        let embedder = DeterministicEmbeddingProvider::new();
        let manager = ParaphraseClusterManager::new_with_persistence(Some(store_path.clone()));
        let tenant = TenantId::new("t-cluster");
        let app = ApplicationId::new("app-cluster");

        let p1 = "Explain how semantic caching reduces latency in distributed systems";
        let p2 = "Describe how semantic caching reduces latency in distributed systems";
        let v1 = embedder.embed(p1).await.unwrap();
        let v2 = embedder.embed(p2).await.unwrap();

        let (cid1, created1) = manager.assign_or_create(&tenant, &app, "k1", p1, &v1, 0.70);
        assert!(created1);
        let (cid2, created2) = manager.assign_or_create(&tenant, &app, "k2", p2, &v2, 0.70);
        assert!(!created2);
        assert_eq!(cid1, cid2);

        // Recalibrate representative
        let recalibrated = manager.recalibrate_representative(&cid1).unwrap();
        assert_eq!(recalibrated.members.len(), 2);
        assert!(recalibrated.version >= 2);

        // Reload from persistent disk file
        let reloaded = ParaphraseClusterManager::new_with_persistence(Some(store_path.clone()));
        assert_eq!(
            reloaded.cluster_for_member("k1").as_deref(),
            Some(cid1.as_str())
        );
        assert_eq!(
            reloaded.cluster_for_member("k2").as_deref(),
            Some(cid1.as_str())
        );

        // Create another compatible cluster and merge them
        let p3 = "Summarize how semantic caching reduces latency in distributed systems";
        let v3 = embedder.embed(p3).await.unwrap();
        let (cid3, _) = reloaded.assign_or_create(&tenant, &app, "k3", p3, &v3, 0.999);
        assert_ne!(cid1, cid3);
        let merged = reloaded.merge_clusters(&cid1, &cid3, 0.60).unwrap();
        assert_eq!(merged.members.len(), 3);
        assert!(reloaded.get_cluster(&cid3).is_none());

        // Add an orthogonal prompt directly under a second group and test split
        let p_other = "Quantum entanglement distributes Bell pairs across fiber optics";
        let v_other = embedder.embed(p_other).await.unwrap();
        let (cid_split_target, _) =
            reloaded.assign_or_create(&tenant, &app, "k-orth", p_other, &v_other, 0.99);
        // Merge only works if compatible, so test split on a cluster with a synthetic divergent member:
        {
            let mut clusters = write_lock(&reloaded.clusters);
            let mut index = write_lock(&reloaded.member_to_cluster);
            let c = clusters.get_mut(&cid_split_target).unwrap();
            c.members.insert(
                "k-orth-2".to_string(),
                ClusterMember {
                    cache_key: "k-orth-2".to_string(),
                    prompt: p1.to_string(),
                    vector: v1.clone(),
                    added_at: Utc::now(),
                },
            );
            index.insert("k-orth-2".to_string(), cid_split_target.clone());
            ParaphraseClusterManager::recompute_centroid_and_medoid(c);
        }
        let split_res = reloaded.split_cluster(&cid_split_target, 0.90).unwrap();
        assert_eq!(split_res.len(), 2);

        // Invalidate cluster by member -> returns all member cache keys
        let evicted = reloaded.invalidate_by_member("k1");
        assert_eq!(evicted.len(), 3);
        assert!(reloaded.get_cluster(&cid1).is_none());

        // Purge remaining tenant clusters
        let purged = reloaded.purge_tenant(&tenant);
        assert!(purged >= 1);
        assert!(reloaded.list_tenant_clusters(&tenant).is_empty());

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
