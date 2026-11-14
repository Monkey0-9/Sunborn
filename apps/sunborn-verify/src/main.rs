use chrono::Utc;
use ed25519_dalek::VerifyingKey;
use std::env;
use std::fs;
use std::process;
use sunborn_core::ReuseCertificate;

fn print_usage() {
    eprintln!(
        r#"Sunborn Attestation Independent Verifier (CLI)
Usage:
    sunborn-verify --certificate <file_or_json> --public-key <hex_or_file>

Options:
    -c, --certificate    Path to certificate JSON file or raw JSON string
    -k, --public-key     32-byte Ed25519 public key (64-char hex string or path to raw key)
    -h, --help           Display this help message
"#
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        return;
    }
    if args.len() < 3 {
        print_usage();
        process::exit(1);
    }

    let mut cert_input: Option<String> = None;
    let mut key_input: Option<String> = None;

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

    match audit_certificate(&cert, &verifying_key) {
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
}
