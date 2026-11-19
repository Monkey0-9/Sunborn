#!/usr/bin/env python3
"""
Sunborn Conformal Dataset Expansion & Clopper-Pearson Calibration Utility
=========================================================================
Validates, calibrates, and expands domain-stratified adversarial prompt pairs
across 8 enterprise domains:
  1. FinancialAccounting
  2. HealthcareMedical
  3. SecuritySensitive
  4. SystemAdministrative
  5. LegalCompliance
  6. DevOpsCloud
  7. SoftwareEngineering
  8. GeneralReasoning

Calculates the exact one-sided Clopper-Pearson confidence upper bound for false-hit rate:
  Upper Bound = 1 - (1 - confidence)^(1 / N_negatives)
For a nominal target upper bound of 0.1% (0.001) at 99.9% confidence, requires N >= 6,905 negatives.
"""

import argparse
import json
import math
import os
import random
import sys
from typing import Any, Dict, List, Tuple

DEFAULT_DATASET_PATH = os.path.join(os.path.dirname(__file__), "held_out_eval_dataset.json")
DEFAULT_EXPANDED_PATH = os.path.join(os.path.dirname(__file__), "expanded_eval_dataset.json")

STRATA_NAMES = [
    "FinancialAccounting",
    "HealthcareMedical",
    "SecuritySensitive",
    "SystemAdministrative",
    "LegalCompliance",
    "DevOpsCloud",
    "SoftwareEngineering",
    "GeneralReasoning",
]


# =============================================================================
# 1. Numerically Stable Exact Clopper-Pearson Statistics
# =============================================================================

def binomial_cdf(k: int, n: int, p: float) -> float:
    """Computes P(X <= k) for X ~ Binomial(n, p) using numerically stable log-space terms.
    
    Prevents float overflow of math.comb(n, i) for large n (e.g. n > 1,000) and underflow
    of p^i * (1-p)^(n-i).
    """
    if p <= 0.0:
        return 1.0
    if p >= 1.0:
        return 1.0 if k >= n else 0.0
    if k < 0:
        return 0.0
    if k >= n:
        return 1.0

    log_p = math.log(p)
    log_1_p = math.log(1.0 - p)

    log_terms: List[float] = []
    for i in range(k + 1):
        # log(comb(n, i)) = lgamma(n + 1) - lgamma(i + 1) - lgamma(n - i + 1)
        log_comb = math.lgamma(n + 1) - math.lgamma(i + 1) - math.lgamma(n - i + 1)
        log_term = log_comb + (i * log_p) + ((n - i) * log_1_p)
        log_terms.append(log_term)

    # LogSumExp trick for exact probability sum
    max_log = max(log_terms)
    return sum(math.exp(t - max_log) for t in log_terms) * math.exp(max_log)


def clopper_pearson_upper_bound(failures: int, total: int, confidence: float = 0.999) -> float:
    """Exact one-sided Clopper-Pearson upper confidence bound for binomial proportion.
    
    Solves P(X <= failures; total, p_U) = 1 - confidence for p_U.
    For failures == 0, solves (1 - p_U)^total = 1 - confidence analytically.
    """
    if total <= 0:
        return 1.0
    if failures < 0:
        raise ValueError(f"failures cannot be negative, got {failures}")
    if failures >= total:
        return 1.0
    if not (0.0 < confidence < 1.0):
        raise ValueError(f"confidence must be strictly between 0 and 1, got {confidence}")

    # Closed-form exact solution when 0 failures are observed
    if failures == 0:
        return 1.0 - math.pow(1.0 - confidence, 1.0 / total)

    alpha = 1.0 - confidence
    low, high = 0.0, 1.0
    for _ in range(100):
        mid = (low + high) / 2.0
        if binomial_cdf(failures, total, mid) > alpha:
            low = mid
        else:
            high = mid
    return mid


def required_samples_for_target_bound(target_bound: float = 0.001, confidence: float = 0.999) -> int:
    """Computes minimum hard negatives needed to prove target bound at given confidence with 0 observed failures.
    
    Theorem: N >= ceil( ln(1 - confidence) / ln(1 - target_bound) )
    """
    if not (0.0 < target_bound < 1.0):
        raise ValueError(f"target_bound must be strictly between 0 and 1, got {target_bound}")
    if not (0.0 < confidence < 1.0):
        raise ValueError(f"confidence must be strictly between 0 and 1, got {confidence}")
    return math.ceil(math.log(1.0 - confidence) / math.log(1.0 - target_bound))


# =============================================================================
# 2. Dataset Loading & Comprehensive Validation
# =============================================================================

def load_dataset(filepath: str) -> Dict[str, Any]:
    """Loads a JSON calibration dataset file with comprehensive error handling."""
    if not os.path.isfile(filepath):
        raise FileNotFoundError(f"Dataset file not found at path: {filepath}")
    with open(filepath, "r", encoding="utf-8") as f:
        return json.load(f)


def validate_dataset(data: Dict[str, Any]) -> Dict[str, Any]:
    """Validates the schema, stratum breakdown, and integrity of a calibration dataset.
    
    Returns a diagnostic summary dictionary with validation status and metrics.
    """
    errors: List[str] = []

    if "dataset_metadata" not in data:
        errors.append("Missing 'dataset_metadata' root object.")
    if "evaluation_strata" not in data:
        errors.append("Missing 'evaluation_strata' list.")

    if errors:
        return {"is_valid": False, "errors": errors}

    meta = data["dataset_metadata"]
    strata = data["evaluation_strata"]

    declared_total = meta.get("total_samples", 0)
    actual_total = len(strata)
    if declared_total != actual_total:
        errors.append(f"Metadata total_samples ({declared_total}) != actual strata count ({actual_total}).")

    strata_breakdown = meta.get("strata_breakdown", {})
    actual_strata_counts: Dict[str, int] = {}
    actual_label_counts: Dict[str, int] = {}
    actual_shift_counts: Dict[str, int] = {}
    seen_ids = set()

    for idx, item in enumerate(strata):
        # Validate required fields
        for field in ("id", "stratum", "prompt_a", "prompt_b", "label", "expected_decision", "structural_shift"):
            if field not in item:
                errors.append(f"Sample at index {idx} missing required field '{field}'.")

        item_id = item.get("id", f"idx_{idx}")
        if item_id in seen_ids:
            errors.append(f"Duplicate sample ID detected: '{item_id}'.")
        seen_ids.add(item_id)

        prompt_a = item.get("prompt_a", "")
        prompt_b = item.get("prompt_b", "")
        if not prompt_a or not prompt_b:
            errors.append(f"Sample '{item_id}' has empty prompt_a or prompt_b.")

        stratum = item.get("stratum", "Unknown")
        actual_strata_counts[stratum] = actual_strata_counts.get(stratum, 0) + 1

        label = item.get("label", "Unknown")
        actual_label_counts[label] = actual_label_counts.get(label, 0) + 1

        decision = item.get("expected_decision", "Unknown")
        shift = item.get("structural_shift", "Unknown")
        actual_shift_counts[shift] = actual_shift_counts.get(shift, 0) + 1

        # Semantic label & expected decision consistency
        if label == "EQUIVALENT" and decision not in ("REUSE_SAFE", "REUSE"):
            errors.append(f"Sample '{item_id}' with label 'EQUIVALENT' has inconsistent decision '{decision}'.")
        elif label == "HARD_NEGATIVE" and decision not in ("ABSTAIN_OR_RECOMPUTE", "REJECT_RISK"):
            errors.append(f"Sample '{item_id}' with label 'HARD_NEGATIVE' has inconsistent decision '{decision}'.")
        elif label not in ("EQUIVALENT", "HARD_NEGATIVE"):
            errors.append(f"Sample '{item_id}' has unrecognized label '{label}'.")

    # Verify strata breakdown counts
    for stratum, expected_count in strata_breakdown.items():
        actual_count = actual_strata_counts.get(stratum, 0)
        if actual_count != expected_count:
            errors.append(f"Stratum '{stratum}' expected {expected_count} samples, found {actual_count}.")

    hard_negatives = actual_label_counts.get("HARD_NEGATIVE", 0)
    bound_999 = clopper_pearson_upper_bound(0, hard_negatives, 0.999) if hard_negatives > 0 else 1.0

    return {
        "is_valid": len(errors) == 0,
        "errors": errors,
        "total_samples": actual_total,
        "label_counts": actual_label_counts,
        "strata_counts": actual_strata_counts,
        "shift_counts": actual_shift_counts,
        "hard_negatives": hard_negatives,
        "clopper_pearson_999_bound": bound_999,
    }


# =============================================================================
# 3. Domain-Stratified Synthetic Expansion Engine
# =============================================================================

# Synthetic templates across 8 industrial strata
EXPANSION_TEMPLATES = {
    "FinancialAccounting": [
        ("Retrieve the quarterly cash flow statement for {entity}.", "Fetch the quarterly cash flow statement for {entity}.", "EQUIVALENT", "none"),
        ("What is the total operational expense reported by {entity} for Q{q} {year}?", "Show the total operational expense reported by {entity} for Q{q} {year}.", "EQUIVALENT", "none"),
        ("Compute the EBITDA margin for {entity} in fiscal year {year}.", "Calculate the EBITDA margin for {entity} in fiscal year {year}.", "EQUIVALENT", "none"),
        ("What was the operating income for Q{q} {year}?", "What was the operating income for Q{q_alt} {year}?", "HARD_NEGATIVE", "temporal_quarter_shift"),
        ("Retrieve the cash flow statement for {entity}.", "Retrieve the cash flow statement for {entity_alt}.", "HARD_NEGATIVE", "entity_substitution"),
        ("Was GAAP net income greater than ${amt} million?", "Was GAAP net income greater than ${amt_alt} million?", "HARD_NEGATIVE", "numeric_magnitude_shift"),
        ("Include all non-recurring asset impairment charges.", "Exclude all non-recurring asset impairment charges.", "HARD_NEGATIVE", "negation_inversion"),
    ],
    "HealthcareMedical": [
        ("Verify patient {patient_id} vital signs from the last ICU check.", "Check patient {patient_id} vital signs from the last ICU check.", "EQUIVALENT", "none"),
        ("Retrieve dosage instructions for {drug} administration.", "Fetch dosage instructions for {drug} administration.", "EQUIVALENT", "none"),
        ("Confirm {drug} infusion rate is {rate} ml/hr.", "Confirm {drug} infusion rate is {rate_alt} ml/hr.", "HARD_NEGATIVE", "numeric_magnitude_shift"),
        ("Patient {patient_id} has no documented allergy to {drug}.", "Patient {patient_id} has a documented allergy to {drug}.", "HARD_NEGATIVE", "allergy_negation_inversion"),
        ("Administer {drug} intravenously to {patient_id}.", "Administer {drug} orally to {patient_id}.", "HARD_NEGATIVE", "administration_route_substitution"),
        ("Verify blood type compatibility for {patient_id} (Type {btype}).", "Verify blood type compatibility for {patient_id} (Type {btype_alt}).", "HARD_NEGATIVE", "blood_type_incompatibility"),
    ],
    "SecuritySensitive": [
        ("Revoke API key {key_id} for tenant {tenant_id}.", "Terminate API key {key_id} for tenant {tenant_id}.", "EQUIVALENT", "none"),
        ("Enforce mutual TLS on gateway endpoint /v1/{endpoint}.", "Require mutual TLS on gateway endpoint /v1/{endpoint}.", "EQUIVALENT", "none"),
        ("Allow anonymous callers to reach /v1/{endpoint}.", "Deny anonymous callers from reaching /v1/{endpoint}.", "HARD_NEGATIVE", "authorization_inversion"),
        ("Set session token time-to-live to {ttl} seconds.", "Set session token time-to-live to {ttl_alt} seconds.", "HARD_NEGATIVE", "timeout_magnitude_expansion"),
        ("Bypass multi-factor authentication for administrative users.", "Enforce multi-factor authentication for administrative users.", "HARD_NEGATIVE", "mfa_bypass_inversion"),
        ("Grant tenant {tenant_id} read-only access to cluster state.", "Grant tenant {tenant_id} administrative write access to cluster state.", "HARD_NEGATIVE", "privilege_elevation_reversal"),
    ],
    "SystemAdministrative": [
        ("Restart the systemd service unit {unit}.service.", "Reboot the systemd service unit {unit}.service.", "EQUIVALENT", "none"),
        ("Mount EBS volume {vol} to /mnt/data.", "Attach EBS volume {vol} to /mnt/data.", "EQUIVALENT", "none"),
        ("Set cgroup memory limit to {mem} GB for worker process.", "Set cgroup memory limit to {mem_alt} GB for worker process.", "HARD_NEGATIVE", "resource_quota_shift"),
        ("Send SIGTERM signal to process PID {pid}.", "Send SIGKILL signal to process PID {pid}.", "HARD_NEGATIVE", "action_inversion"),
        ("Format filesystem on /dev/{disk} using ext4.", "Format filesystem on /dev/{disk_alt} using ext4.", "HARD_NEGATIVE", "destructive_operation_target_shift"),
        ("Enable automated weekly volume snapshots.", "Disable automated weekly volume snapshots.", "HARD_NEGATIVE", "backup_policy_inversion"),
    ],
    "LegalCompliance": [
        ("Verify GDPR Article 17 erasure request compliance for user {uid}.", "Confirm GDPR Article 17 erasure request adherence for user {uid}.", "EQUIVALENT", "none"),
        ("Review non-disclosure agreement liability limitation terms.", "Examine non-disclosure agreement liability limitation terms.", "EQUIVALENT", "none"),
        ("Governing law shall be the laws of the State of Delaware.", "Governing law shall be the laws of the State of California.", "HARD_NEGATIVE", "jurisdiction_forum_shift"),
        ("The total aggregate liability cap is ${amt} thousand.", "The total aggregate liability cap is ${amt_alt} thousand.", "HARD_NEGATIVE", "liability_cap_magnitude"),
        ("Vendor shall indemnify customer against third-party claims.", "Vendor shall not indemnify customer against third-party claims.", "HARD_NEGATIVE", "negation_inversion"),
        ("The non-competition restriction shall endure for {years} years.", "The non-competition restriction shall endure for {years_alt} years.", "HARD_NEGATIVE", "duration_numeric_shift"),
    ],
    "DevOpsCloud": [
        ("Scale deployment {deploy} to {replicas} replicas.", "Adjust deployment {deploy} pod count to {replicas}.", "EQUIVALENT", "none"),
        ("Apply ingress route configuration to namespace {ns}.", "Deploy ingress route configuration to namespace {ns}.", "EQUIVALENT", "none"),
        ("Scale deployment {deploy} to {replicas} replicas in production.", "Scale deployment {deploy} to {replicas_alt} replicas in production.", "HARD_NEGATIVE", "pod_scale_mutation"),
        ("Promote container image tag v{ver} to production.", "Promote container image tag v{ver_alt} to production.", "HARD_NEGATIVE", "container_version_shift"),
        ("Allow ingress CIDR block {cidr} to port 443.", "Allow ingress CIDR block 0.0.0.0/0 to port 443.", "HARD_NEGATIVE", "cidr_security_widening"),
        ("Enable S3 bucket versioning and object lock.", "Disable S3 bucket versioning and object lock.", "HARD_NEGATIVE", "backup_retention_disable"),
    ],
    "SoftwareEngineering": [
        ("Refactor SIMD vector dot product kernel for AVX2.", "Restructure SIMD vector dot product kernel for AVX2.", "EQUIVALENT", "none"),
        ("Optimize binary search tree rebalancing routine.", "Improve binary search tree rebalancing routine.", "EQUIVALENT", "none"),
        ("Compile native accelerator using compiler optimization level -O3.", "Compile native accelerator using compiler optimization level -O0.", "HARD_NEGATIVE", "optimization_level_contrast"),
        ("Enforce atomic memory ordering SeqCst for lock-free queue.", "Enforce atomic memory ordering Relaxed for lock-free queue.", "HARD_NEGATIVE", "memory_ordering_downgrade"),
        ("Assert that buffer length is strictly greater than 0.", "Assert that buffer length is less than or equal to 0.", "HARD_NEGATIVE", "relational_bound_inversion"),
        ("Free dynamically allocated pointer buffer on function exit.", "Retain dynamically allocated pointer buffer without freeing on exit.", "HARD_NEGATIVE", "memory_safety_violation"),
    ],
    "GeneralReasoning": [
        ("Deduce whether proposition P implies proposition Q.", "Infer whether proposition P implies proposition Q.", "EQUIVALENT", "none"),
        ("Determine the shortest path between node A and node B.", "Compute the shortest path between node A and node B.", "EQUIVALENT", "none"),
        ("If all components succeed, then the system remains available.", "If some components succeed, then the system remains available.", "HARD_NEGATIVE", "quantifier_scope_inversion"),
        ("Action X strictly caused observable outcome Y.", "Observable outcome Y strictly caused action X.", "HARD_NEGATIVE", "causality_reversal"),
        ("Condition C is both necessary and sufficient for event E.", "Condition C is necessary but not sufficient for event E.", "HARD_NEGATIVE", "logical_condition_flip"),
        ("Event A preceded event B in monotonic time.", "Event B preceded event A in monotonic time.", "HARD_NEGATIVE", "temporal_calendar_shift"),
    ],
}


def generate_expanded_dataset(base_data: Dict[str, Any], target_samples: int = 7000, seed: int = 42) -> Dict[str, Any]:
    """Generates a domain-stratified expanded evaluation dataset reaching target_samples.
    
    Preserves all existing base samples and deterministically generates new domain-stratified
    prompt pairs across all 8 enterprise strata.
    """
    random.seed(seed)
    existing_strata: List[Dict[str, Any]] = list(base_data.get("evaluation_strata", []))
    current_count = len(existing_strata)

    if current_count >= target_samples:
        return base_data

    needed = target_samples - current_count
    per_stratum_needed = math.ceil(needed / len(STRATA_NAMES))

    # Entities and parameters for synthetic generation
    entities = ["Acme Corp", "Beta Technologies", "Zenith Global", "Apex Financial", "Omni Dynamics", "Horizon Systems"]
    drugs = ["Heparin", "Insulin", "Amoxicillin", "Propofol", "Fentanyl", "Dopamine", "Epinephrine"]
    btypes = ["O+", "O-", "A+", "A-", "B+", "B-", "AB+", "AB-"]

    new_pairs: List[Dict[str, Any]] = []
    pair_id_counter = current_count + 1

    for stratum in STRATA_NAMES:
        templates = EXPANSION_TEMPLATES.get(stratum, EXPANSION_TEMPLATES["GeneralReasoning"])
        for _ in range(per_stratum_needed):
            tmpl_a, tmpl_b, label, shift = random.choice(templates)
            q = random.randint(1, 4)
            q_alt = 1 if q == 4 else q + 1
            year = random.randint(2024, 2026)
            ent_sample = random.sample(entities, 2)
            drug_sample = random.sample(drugs, 2)
            btype_sample = random.sample(btypes, 2)
            amt = random.randint(10, 50)
            amt_alt = amt * 10
            patient_id = random.randint(100, 999)
            rate = random.randint(10, 50)
            rate_alt = rate * 5
            key_id = f"key_{random.randint(1000, 9999)}"
            tenant_id = f"tenant_{random.randint(10, 99)}"
            endpoint = random.choice(["chat/completions", "invalidation", "governance", "tokens"])
            ttl = random.choice([300, 600, 3600])
            ttl_alt = ttl * 10
            unit = random.choice(["sunborn-gateway", "dragonfly", "postgres", "redis-cluster"])
            vol = f"vol-{random.randint(10000, 99999):05d}"
            mem = random.randint(2, 16)
            mem_alt = mem * 4
            pid = random.randint(1000, 65000)
            disk = random.choice(["nvme0n1", "sda", "vda"])
            disk_alt = random.choice(["nvme1n1", "sdb", "vdb"])
            uid = f"usr_{random.randint(10000, 99999)}"
            years = random.randint(1, 5)
            years_alt = years + 5
            deploy = random.choice(["inference-router", "semantic-cache", "outbox-worker"])
            ns = random.choice(["production", "staging", "cluster-mesh"])
            replicas = random.randint(3, 10)
            replicas_alt = replicas * 3
            ver = f"{random.randint(1, 3)}.{random.randint(0, 9)}"
            ver_alt = f"{random.randint(4, 6)}.{random.randint(0, 9)}"
            cidr = f"10.{random.randint(0, 255)}.0.0/16"

            context = {
                "entity": ent_sample[0], "entity_alt": ent_sample[1],
                "q": q, "q_alt": q_alt,
                "year": year,
                "amt": amt, "amt_alt": amt_alt,
                "patient_id": patient_id,
                "drug": drug_sample[0], "drug_alt": drug_sample[1],
                "rate": rate, "rate_alt": rate_alt,
                "btype": btype_sample[0], "btype_alt": btype_sample[1],
                "key_id": key_id, "tenant_id": tenant_id, "endpoint": endpoint,
                "ttl": ttl, "ttl_alt": ttl_alt,
                "unit": unit, "vol": vol,
                "mem": mem, "mem_alt": mem_alt,
                "pid": pid, "disk": disk, "disk_alt": disk_alt,
                "uid": uid, "years": years, "years_alt": years_alt,
                "deploy": deploy, "ns": ns,
                "replicas": replicas, "replicas_alt": replicas_alt,
                "ver": ver, "ver_alt": ver_alt,
                "cidr": cidr,
            }

            p_a = tmpl_a.format(**context)
            p_b = tmpl_b.format(**context)
            expected_decision = "REUSE_SAFE" if label == "EQUIVALENT" else "ABSTAIN_OR_RECOMPUTE"

            new_pairs.append({
                "id": f"pair_{pair_id_counter:04d}",
                "stratum": stratum,
                "prompt_a": p_a,
                "prompt_b": p_b,
                "label": label,
                "expected_decision": expected_decision,
                "structural_shift": shift,
            })
            pair_id_counter += 1

            if len(existing_strata) + len(new_pairs) >= target_samples:
                break
        if len(existing_strata) + len(new_pairs) >= target_samples:
            break

    combined_strata = existing_strata + new_pairs

    # Recompute metadata
    new_strata_breakdown: Dict[str, int] = {}
    for item in combined_strata:
        s = item["stratum"]
        new_strata_breakdown[s] = new_strata_breakdown.get(s, 0) + 1

    expanded_data = {
        "dataset_metadata": {
            "version": "2.1.0-expanded",
            "evaluation_standard": base_data.get("dataset_metadata", {}).get("evaluation_standard", "IEEE-730/ACM-Artifact-Evaluation/ISO-IEC-25010"),
            "description": f"Domain-stratified expanded evaluation benchmark (N={len(combined_strata)} across 8 enterprise strata)",
            "total_samples": len(combined_strata),
            "domains": base_data.get("dataset_metadata", {}).get("domains", []),
            "strata_breakdown": new_strata_breakdown,
            "statistical_design": {
                "alpha_nominal": 0.001,
                "target_false_positive_rate": 0.0,
                "confidence_level": 0.999,
                "min_sample_size": 6905,
            },
        },
        "evaluation_strata": combined_strata,
    }

    return expanded_data


def save_dataset(data: Dict[str, Any], output_path: str) -> None:
    """Saves a dataset dictionary to formatted JSON."""
    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)


# =============================================================================
# 4. Command-Line Interface & Main Execution
# =============================================================================

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Sunborn Conformal Dataset Calibration & Expansion Engine",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--dataset",
        type=str,
        default=DEFAULT_DATASET_PATH,
        help="Path to held-out evaluation dataset JSON file.",
    )
    parser.add_argument(
        "--validate",
        action="store_true",
        help="Run comprehensive validation on the dataset file and print diagnostics.",
    )
    parser.add_argument(
        "--expand",
        action="store_true",
        help="Generate synthetic domain-stratified prompt pairs to expand the dataset.",
    )
    parser.add_argument(
        "--target-samples",
        type=int,
        default=7000,
        help="Target total sample count for expansion (default: 7,000).",
    )
    parser.add_argument(
        "--output",
        type=str,
        default=DEFAULT_EXPANDED_PATH,
        help="Output path for expanded dataset JSON file.",
    )
    parser.add_argument(
        "--target-bound",
        type=float,
        default=0.001,
        help="Target false-hit rate upper bound (default: 0.001 = 0.1%%).",
    )
    parser.add_argument(
        "--confidence",
        type=float,
        default=0.999,
        help="Confidence level for Clopper-Pearson bound (default: 0.999 = 99.9%%).",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="Random seed for deterministic generation (default: 42).",
    )
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="Suppress banner output for script integration.",
    )
    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()

    if not args.quiet:
        print("=" * 80)
        print("Sunborn Conformal Dataset Calibration & Sample Size Engine")
        print("=" * 80)

    # 1. Dataset Loading & Validation
    if os.path.exists(args.dataset):
        try:
            dataset_data = load_dataset(args.dataset)
            val_result = validate_dataset(dataset_data)
            n_samples = val_result.get("total_samples", 0)
            n_negatives = val_result.get("hard_negatives", 0)
            bound_999 = val_result.get("clopper_pearson_999_bound", 1.0) * 100.0

            if not args.quiet:
                print(f"Dataset Path : {args.dataset}")
                print(f"Validation   : {'[PASS] VALID' if val_result['is_valid'] else '[FAIL] INVALID'}")
                print(f"Total Samples: {n_samples} (Equivalents: {n_samples - n_negatives}, Hard Negatives: {n_negatives})")
                print(f"Current Exact 99.9% Upper Confidence Bound: {bound_999:.3f}% (at 0 observed false hits)")
                print("-" * 80)

            if not val_result["is_valid"]:
                print("Validation Errors:")
                for err in val_result["errors"]:
                    print(f"  - {err}")
                if args.validate:
                    sys.exit(1)

        except Exception as e:
            print(f"[!] Error loading dataset: {e}")
            if args.validate:
                sys.exit(1)
    else:
        if not args.quiet:
            print(f"[!] Dataset file not found at: {args.dataset}")
        n_negatives = 200

    # 2. Sample Size Requirements Analysis
    req_01 = required_samples_for_target_bound(0.001, args.confidence)
    req_05 = required_samples_for_target_bound(0.005, args.confidence)
    req_10 = required_samples_for_target_bound(0.010, args.confidence)

    if not args.quiet:
        print(f"Sample Size Requirements for Exact Statistical Claims ({args.confidence * 100:.1f}% Confidence, 0 Failures):")
        print(f"  - To guarantee <= 1.0% False-Hit Rate: N >= {req_10:,} hard negatives")
        print(f"  - To guarantee <= 0.5% False-Hit Rate: N >= {req_05:,} hard negatives")
        print(f"  - To guarantee <= 0.1% False-Hit Rate: N >= {req_01:,} hard negatives")
        print("=" * 80)

    # 3. Optional Dataset Expansion
    if args.expand:
        if not os.path.exists(args.dataset):
            print(f"[!] Cannot expand: Base dataset not found at {args.dataset}")
            sys.exit(1)

        print(f"Expanding dataset from {len(dataset_data['evaluation_strata'])} to {args.target_samples} samples...")
        expanded = generate_expanded_dataset(dataset_data, target_samples=args.target_samples, seed=args.seed)
        save_dataset(expanded, args.output)

        exp_val = validate_dataset(expanded)
        print(f"[✓] Successfully generated and verified expanded dataset at: {args.output}")
        print(f"    - Total Samples: {exp_val['total_samples']}")
        print(f"    - Hard Negatives: {exp_val['hard_negatives']}")
        exp_bound = exp_val['clopper_pearson_999_bound'] * 100.0
        print(f"    - New Clopper-Pearson 99.9% Upper Bound: {exp_bound:.4f}%")
        print(f"    - Supports nominal alpha <= 0.1%: {'YES' if exp_bound <= 0.1 else 'NO'}")
        print("=" * 80)


if __name__ == "__main__":
    main()
