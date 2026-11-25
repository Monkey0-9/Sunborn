use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::process;
use sunborn_core::{FiveGateEvidence, ReuseCertificate, SourceVersionSnapshot};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthoritativeDependencyState {
    #[serde(default)]
    pub current_time: Option<DateTime<Utc>>,
    pub knowledge_epoch: u64,
    #[serde(default)]
    pub source_snapshots: Vec<SourceVersionSnapshot>,
    #[serde(default)]
    pub expected_tenant_id: Option<String>,
    #[serde(default)]
    pub expected_application_id: Option<String>,
    #[serde(default)]
    pub expected_model: Option<String>,
    #[serde(default)]
    pub revoked_tenant_ids: Vec<String>,
    #[serde(default)]
    pub authoritative_permission_version: Option<u64>,
    #[serde(default)]
    pub certificate_permission_version: Option<u64>,
}

fn print_usage() {
    eprintln!(
        r#"Sunborn Attestation Independent Verifier (CLI)
Usage:
    sunborn-verify --certificate <file_or_json> --public-key <hex_or_file> [--state <file_or_json>] [--evidence <file_or_json>]
    sunborn-verify --openapi

Options:
    -c, --certificate    Path to certificate JSON file or raw JSON string
    -k, --public-key     32-byte Ed25519 public key (64-char hex string or path to raw key)
    -s, --state          Optional authoritative dependency & revocation state JSON (file or string)
    -e, --evidence       Optional FiveGateEvidence JSON (file or string)
    --openapi            Verify OpenAPI specification contract parity with gateway routes
    -h, --help           Display this help message
"#
    );
}

pub fn verify_openapi_contract(repo_root: &std::path::Path) -> Result<usize, String> {
    let openapi_path = repo_root.join("docs").join("api").join("openapi.yaml");
    let lib_path = repo_root
        .join("crates")
        .join("sunborn-gateway")
        .join("src")
        .join("lib.rs");

    if !openapi_path.exists() {
        return Err(format!(
            "Missing canonical OpenAPI spec at {}",
            openapi_path.display()
        ));
    }
    if !lib_path.exists() {
        return Err(format!(
            "Missing gateway source lib.rs at {}",
            lib_path.display()
        ));
    }

    let openapi_content = fs::read_to_string(&openapi_path)
        .map_err(|e| format!("Failed to read openapi.yaml: {e}"))?;
    let lib_content =
        fs::read_to_string(&lib_path).map_err(|e| format!("Failed to read lib.rs: {e}"))?;

    let gateway_routes = extract_gateway_routes(&lib_content);
    if gateway_routes.is_empty() {
        return Err("Could not extract any routes from gateway lib.rs".to_string());
    }

    // Compare real path sets in both directions. The previous implementation
    // searched for each route as a substring of the whole spec, which a comment
    // or an unrelated longer path would satisfy, and it read only single-line
    // `.route(` calls, silently ignoring multi-line registrations.
    let spec_paths = extract_spec_paths(&openapi_content);
    if spec_paths.is_empty() {
        return Err("Could not extract any paths from openapi.yaml".to_string());
    }

    let spec_normalised: std::collections::BTreeSet<String> =
        spec_paths.iter().map(|path| normalise_path(path)).collect();
    let route_normalised: std::collections::BTreeSet<String> = gateway_routes
        .iter()
        .map(|path| normalise_path(path))
        .collect();

    let missing: Vec<&String> = route_normalised
        .iter()
        .filter(|route| !spec_normalised.contains(*route))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "Gateway routes missing from openapi.yaml: {}",
            missing
                .iter()
                .map(|route| route.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let orphaned: Vec<&String> = spec_normalised
        .iter()
        .filter(|path| !route_normalised.contains(*path))
        .collect();
    if !orphaned.is_empty() {
        return Err(format!(
            "openapi.yaml documents paths with no gateway route: {}",
            orphaned
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    Ok(route_normalised.len())
}

/// Collects every `.route("...", ...)` path, including registrations that span
/// several lines.
fn extract_gateway_routes(source: &str) -> std::collections::BTreeSet<String> {
    use std::collections::BTreeSet;

    let mut routes = BTreeSet::new();
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while let Some(offset) = source[cursor..].find(".route(") {
        let mut index = cursor + offset + ".route(".len();
        while index < bytes.len() && bytes[index].is_ascii_whitespace() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'"' {
            let path_start = index + 1;
            let mut end = path_start;
            while end < bytes.len() && bytes[end] != b'"' {
                end += 1;
            }
            let route = &source[path_start..end];
            if route.starts_with('/') {
                routes.insert(route.to_string());
            }
        }
        cursor = cursor + offset + ".route(".len();
    }

    routes
}

/// Collects top-level path keys from the OpenAPI document. Path keys sit at a
/// two-space indent inside `paths:`, which keeps this independent of a YAML
/// dependency in a supply-chain-sensitive verifier.
fn extract_spec_paths(spec: &str) -> std::collections::BTreeSet<String> {
    use std::collections::BTreeSet;

    let mut paths = BTreeSet::new();
    for line in spec.lines() {
        let trimmed_end = line.trim_end();
        if trimmed_end.starts_with("  /") && trimmed_end.ends_with(':') {
            paths.insert(trimmed_end.trim().trim_end_matches(':').to_string());
        }
    }
    paths
}

/// Reduces a path so `/v1/graph/nodes/{*id}` and `/v1/graph/nodes/{id}` compare
/// equal: parameter names are not part of the routing contract.
fn normalise_path(path: &str) -> String {
    let mut normalised = String::with_capacity(path.len());
    let mut inside_parameter = false;
    for character in path.chars() {
        match character {
            '{' => {
                inside_parameter = true;
                normalised.push('{');
            }
            '}' => {
                if inside_parameter {
                    normalised.push('}');
                }
                inside_parameter = false;
            }
            _ if inside_parameter => {}
            other => normalised.push(other),
        }
    }
    normalised
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return;
    }
    if args.iter().any(|a| a == "--openapi" || a == "openapi") {
        let repo_root = std::env::current_dir().unwrap_or_default();
        match verify_openapi_contract(&repo_root) {
            Ok(count) => {
                println!("OK: Native Rust verification passed: {count} gateway routes verified against OpenAPI contract.");
                return;
            }
            Err(e) => {
                eprintln!("ERROR: OpenAPI verification failed: {e}");
                process::exit(1);
            }
        }
    }
    if args.len() < 3 {
        print_usage();
        process::exit(1);
    }

    let mut cert_input: Option<String> = None;
    let mut key_input: Option<String> = None;
    let mut state_input: Option<String> = None;
    let mut evidence_input: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--certificate" if i + 1 < args.len() => {
                cert_input = Some(args[i + 1].clone());
                i += 1;
            }
            "-k" | "--public-key" if i + 1 < args.len() => {
                key_input = Some(args[i + 1].clone());
                i += 1;
            }
            "-s" | "--state" if i + 1 < args.len() => {
                state_input = Some(args[i + 1].clone());
                i += 1;
            }
            "-e" | "--evidence" if i + 1 < args.len() => {
                evidence_input = Some(args[i + 1].clone());
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    let cert_raw = match cert_input {
        Some(path_or_json) => {
            if let Ok(content) = fs::read_to_string(&path_or_json) {
                content
            } else {
                path_or_json
            }
        }
        None => {
            eprintln!("Error: Missing --certificate argument");
            process::exit(1);
        }
    };

    let key_raw = match key_input {
        Some(path_or_hex) => {
            if let Ok(content) = fs::read_to_string(&path_or_hex) {
                content.trim().to_string()
            } else {
                path_or_hex.trim().to_string()
            }
        }
        None => {
            eprintln!("Error: Missing --public-key argument");
            process::exit(1);
        }
    };

    // 1. Parse ReuseCertificate JSON
    let cert: ReuseCertificate = match serde_json::from_str(&cert_raw) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: Failed to parse ReuseCertificate JSON: {e}");
            process::exit(1);
        }
    };

    // 2. Parse Ed25519 VerifyingKey
    let key_bytes = match hex::decode(&key_raw) {
        Ok(b) if b.len() == 32 => {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&b);
            arr
        }
        _ => {
            eprintln!("Error: Public key must be a valid 64-character hex string representing a 32-byte Ed25519 public key");
            process::exit(1);
        }
    };

    let verifying_key = match VerifyingKey::from_bytes(&key_bytes) {
        Ok(vk) => vk,
        Err(e) => {
            eprintln!("Error: Malformed Ed25519 public key: {e}");
            process::exit(1);
        }
    };

    let state_opt: Option<AuthoritativeDependencyState> = match state_input {
        Some(path_or_json) => {
            let raw = fs::read_to_string(&path_or_json).unwrap_or(path_or_json);
            match serde_json::from_str(&raw) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("Error: Failed to parse --state JSON: {e}");
                    process::exit(1);
                }
            }
        }
        None => None,
    };

    let evidence_opt: Option<FiveGateEvidence> = match evidence_input {
        Some(path_or_json) => {
            let raw = fs::read_to_string(&path_or_json).unwrap_or(path_or_json);
            match serde_json::from_str(&raw) {
                Ok(ev) => Some(ev),
                Err(e) => {
                    eprintln!("Error: Failed to parse --evidence JSON: {e}");
                    process::exit(1);
                }
            }
        }
        None => None,
    };

    match audit_certificate_with_authority(
        &cert,
        &verifying_key,
        state_opt.as_ref(),
        evidence_opt.as_ref(),
    ) {
        Ok(_) => {
            println!(
                "VERDICT: PASSED — Certificate is cryptographically authentic and admissible."
            );
            process::exit(0);
        }
        Err(err) => {
            eprintln!("VERDICT: FAILED — {err}");
            process::exit(1);
        }
    }
}

pub fn audit_certificate(
    cert: &ReuseCertificate,
    verifying_key: &VerifyingKey,
) -> Result<(), String> {
    println!("=== Sunborn Attestation Independent Audit Report ===");
    println!("Certificate ID : {}", cert.certificate_id);
    println!("Tenant ID      : {}", cert.tenant_id.0);
    println!("Application ID : {}", cert.application_id.0);
    println!("Model          : {}", cert.model_name);
    println!("Algorithm      : {:?}", cert.signature_algorithm);
    println!(
        "Key ID         : {}",
        if cert.public_key_id.is_empty() {
            "default"
        } else {
            &cert.public_key_id
        }
    );
    println!("Merkle Root    : {}", cert.merkle_root);
    println!("Issued At      : {}", cert.validity_window.issued_at);
    println!("Expires At     : {}", cert.validity_window.expires_at);

    print!("  [1/3] Merkle Tree Structure Recalculation: ");
    let expected_root = ReuseCertificate::compute_merkle_root(
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

    if cert.merkle_root != expected_root {
        println!(
            "TAMPERED! Expected {}, got {}",
            expected_root, cert.merkle_root
        );
        return Err(format!(
            "Merkle root mismatch: expected {expected_root}, got {}",
            cert.merkle_root
        ));
    }
    println!("VALID (Match: {})", cert.merkle_root);

    print!("  [2/3] Asymmetric Ed25519 Digital Signature: ");
    if let Err(e) = cert.verify_signature_ed25519(verifying_key) {
        println!("INVALID ({e})");
        return Err(format!("Ed25519 signature verification failed: {e}"));
    }
    println!("VERIFIED (Authentic non-repudiation proof)");

    print!("  [3/3] Temporal Freshness Window: ");
    let now = Utc::now();
    if now > cert.validity_window.expires_at {
        println!("EXPIRED (Expired at {})", cert.validity_window.expires_at);
        return Err(format!(
            "Certificate expired at {}",
            cert.validity_window.expires_at
        ));
    }
    let remaining_secs = (cert.validity_window.expires_at - now).num_seconds();
    println!("ACTIVE (Expires in {remaining_secs}s)");

    println!("--------------------------------------------------");
    Ok(())
}

/// Independent gateway-free verification combining:
/// 1. Merkle root & Ed25519 signature authenticity (`audit_certificate`)
/// 2. Authoritative current-state comparison (epochs, source `content_hash`, tenant revocations, scope bindings)
/// 3. Unified five-gate evidence integrity (`G1 && G2 && G3 && G4 && G5`)
pub fn audit_certificate_with_authority(
    cert: &ReuseCertificate,
    verifying_key: &VerifyingKey,
    auth_state: Option<&AuthoritativeDependencyState>,
    five_gates: Option<&FiveGateEvidence>,
) -> Result<(), String> {
    audit_certificate(cert, verifying_key)?;

    if let Some(state) = auth_state {
        if let Some(check_time) = state.current_time {
            if check_time > cert.validity_window.expires_at {
                return Err(format!(
                    "Certificate expired against authoritative clock ({} > {})",
                    check_time, cert.validity_window.expires_at
                ));
            }
        }
        if state
            .revoked_tenant_ids
            .iter()
            .any(|t| t == &cert.tenant_id.0)
        {
            return Err(format!(
                "Tenant '{}' is marked revoked in authoritative state",
                cert.tenant_id.0
            ));
        }
        if let Some(ref exp_tenant) = state.expected_tenant_id {
            if &cert.tenant_id.0 != exp_tenant {
                return Err(format!(
                    "Tenant mismatch: certificate binds '{}', expected '{}'",
                    cert.tenant_id.0, exp_tenant
                ));
            }
        }
        if let Some(ref exp_app) = state.expected_application_id {
            if &cert.application_id.0 != exp_app {
                return Err(format!(
                    "Application mismatch: certificate binds '{}', expected '{}'",
                    cert.application_id.0, exp_app
                ));
            }
        }
        if let Some(ref exp_model) = state.expected_model {
            if &cert.model_name != exp_model {
                return Err(format!(
                    "Model mismatch: certificate binds '{}', expected '{}'",
                    cert.model_name, exp_model
                ));
            }
        }
        if let (Some(auth_perm), Some(cert_perm)) = (
            state.authoritative_permission_version,
            state.certificate_permission_version,
        ) {
            if cert_perm < auth_perm {
                return Err(format!(
                    "Permission version revoked/stale: certificate v{} < authoritative v{}",
                    cert_perm, auth_perm
                ));
            }
        }
        if cert.validity_window.monotonic_epoch_lease < state.knowledge_epoch {
            return Err(format!(
                "Knowledge epoch stale: certificate lease {} < authoritative epoch {}",
                cert.validity_window.monotonic_epoch_lease, state.knowledge_epoch
            ));
        }
        let auth_map: HashMap<_, _> = state
            .source_snapshots
            .iter()
            .map(|s| (s.source_id, s))
            .collect();
        for cert_snap in &cert.source_snapshots {
            if let Some(auth_snap) = auth_map.get(&cert_snap.source_id) {
                if cert_snap.snapshot_epoch < auth_snap.snapshot_epoch {
                    return Err(format!(
                        "Source {} epoch stale: cert epoch {} < authoritative epoch {}",
                        cert_snap.source_id, cert_snap.snapshot_epoch, auth_snap.snapshot_epoch
                    ));
                }
                if !auth_snap.content_hash.is_empty()
                    && !cert_snap.content_hash.is_empty()
                    && cert_snap.content_hash != auth_snap.content_hash
                {
                    return Err(format!(
                        "Source {} content_hash changed: cert '{}' != authoritative '{}'",
                        cert_snap.source_id, cert_snap.content_hash, auth_snap.content_hash
                    ));
                }
            }
        }
    }

    if let Some(evidence) = five_gates {
        let all_passed = evidence.g1_identity_equivalence.passed
            && evidence.g2_context_authorization.passed
            && evidence.g3_dependency_freshness.passed
            && evidence.g4_execution_effects.passed
            && evidence.g5_evidence_policy.passed;
        if evidence.eligible_reuse != all_passed {
            return Err(
                "FiveGateEvidence integrity violation: eligible_reuse flag disagrees with gate conjuncts"
                    .to_string(),
            );
        }
        if !evidence.eligible_reuse {
            return Err(format!(
                "FiveGateEvidence rejected reuse (G1={}, G2={}, G3={}, G4={}, G5={})",
                evidence.g1_identity_equivalence.passed,
                evidence.g2_context_authorization.passed,
                evidence.g3_dependency_freshness.passed,
                evidence.g4_execution_effects.passed,
                evidence.g5_evidence_policy.passed,
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use ed25519_dalek::SigningKey;
    use sunborn_core::{
        ApplicationId, SafetyEvidenceRecord, SourceVersionSnapshot, TenantId, ValidityWindow,
    };
    use uuid::Uuid;

    #[test]
    fn test_audit_certificate_valid_and_tamper_detection() {
        let seed = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let cert = ReuseCertificate::new_signed_ed25519(
            TenantId::new("tenant-audit"),
            ApplicationId::new("app-audit"),
            "req-hash-audit",
            "claude-3-5-sonnet",
            128,
            vec![SourceVersionSnapshot {
                source_id: Uuid::new_v4(),
                snapshot_epoch: 1,
                content_hash: "hash-doc-audit".to_string(),
            }],
            SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.01,
                calibrated_risk_bound: 0.05,
                heuristic_risk_score: 0.0,
            },
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
                monotonic_epoch_lease: 1,
            },
            &signing_key,
            "key-01",
        );

        // 1. Valid certificate passes
        assert!(audit_certificate(&cert, &verifying_key).is_ok());

        // 2. Tampered signature fails
        let mut tampered_sig_cert = cert.clone();
        tampered_sig_cert.signature = "deadbeef".to_string();
        assert!(audit_certificate(&tampered_sig_cert, &verifying_key).is_err());

        // 3. Tampered Merkle root fails
        let mut tampered_merkle_cert = cert.clone();
        tampered_merkle_cert.merkle_root =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        assert!(audit_certificate(&tampered_merkle_cert, &verifying_key).is_err());
    }

    #[test]
    fn test_native_openapi_verification() {
        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir.parent().unwrap().parent().unwrap();
        let res = verify_openapi_contract(repo_root);
        assert!(
            res.is_ok(),
            "OpenAPI contract verification failed: {:?}",
            res
        );
        assert!(res.unwrap() > 20, "Should have verified all gateway routes");
    }

    #[test]
    fn test_audit_with_authoritative_state_and_five_gate_evidence() {
        let seed = [7u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();
        let src_id = Uuid::new_v4();

        let cert = ReuseCertificate::new_signed_ed25519(
            TenantId::new("tenant-auth"),
            ApplicationId::new("app-auth"),
            "req-hash-auth",
            "gpt-4o",
            64,
            vec![SourceVersionSnapshot {
                source_id: src_id,
                snapshot_epoch: 5,
                content_hash: "sha256:v5".to_string(),
            }],
            SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.01,
                calibrated_risk_bound: 0.05,
                heuristic_risk_score: 0.0,
            },
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
                monotonic_epoch_lease: 5,
            },
            &signing_key,
            "key-02",
        );

        let good_state = AuthoritativeDependencyState {
            current_time: Some(Utc::now()),
            knowledge_epoch: 5,
            source_snapshots: vec![SourceVersionSnapshot {
                source_id: src_id,
                snapshot_epoch: 5,
                content_hash: "sha256:v5".to_string(),
            }],
            expected_tenant_id: Some("tenant-auth".to_string()),
            expected_application_id: Some("app-auth".to_string()),
            expected_model: Some("gpt-4o".to_string()),
            revoked_tenant_ids: vec![],
            authoritative_permission_version: Some(3),
            certificate_permission_version: Some(3),
        };
        let good_evidence = FiveGateEvidence::evaluate(
            sunborn_core::GateCheckResult::pass("G1_OK"),
            sunborn_core::GateCheckResult::pass("G2_OK"),
            sunborn_core::GateCheckResult::pass("G3_OK"),
            sunborn_core::GateCheckResult::pass("G4_OK"),
            sunborn_core::GateCheckResult::pass("G5_OK"),
        );

        // 1. Fresh state + passing 5 gates -> passes
        assert!(audit_certificate_with_authority(
            &cert,
            &verifying_key,
            Some(&good_state),
            Some(&good_evidence),
        )
        .is_ok());

        // 2. Changed authoritative content_hash -> fails independently of gateway
        let mut mutated_hash = good_state.clone();
        mutated_hash.source_snapshots[0].content_hash = "sha256:v6-mutated".to_string();
        assert!(audit_certificate_with_authority(
            &cert,
            &verifying_key,
            Some(&mutated_hash),
            Some(&good_evidence),
        )
        .is_err());

        // 3. Stale permission version -> fails
        let mut stale_perm = good_state.clone();
        stale_perm.authoritative_permission_version = Some(4);
        assert!(audit_certificate_with_authority(
            &cert,
            &verifying_key,
            Some(&stale_perm),
            Some(&good_evidence),
        )
        .is_err());

        // 4. Revoked tenant -> fails
        let mut revoked = good_state.clone();
        revoked.revoked_tenant_ids.push("tenant-auth".to_string());
        assert!(audit_certificate_with_authority(
            &cert,
            &verifying_key,
            Some(&revoked),
            Some(&good_evidence),
        )
        .is_err());

        // 5. Failed G4 or tampered eligible_reuse flag -> fails
        let bad_evidence = FiveGateEvidence::evaluate(
            sunborn_core::GateCheckResult::pass("G1_OK"),
            sunborn_core::GateCheckResult::pass("G2_OK"),
            sunborn_core::GateCheckResult::pass("G3_OK"),
            sunborn_core::GateCheckResult::fail("G4_MUTATION"),
            sunborn_core::GateCheckResult::pass("G5_OK"),
        );
        assert!(audit_certificate_with_authority(
            &cert,
            &verifying_key,
            Some(&good_state),
            Some(&bad_evidence),
        )
        .is_err());
    }
}
