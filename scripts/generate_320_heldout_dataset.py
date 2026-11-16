#!/usr/bin/env python3
"""
Generate a 320-sample pre-registered held-out evaluation dataset for Sunborn.
Covers 8 strata across 13 failure dimensions:
- FinancialAccounting (40 pairs: 15 equivalent, 25 hard negative)
- HealthcareMedical (40 pairs: 15 equivalent, 25 hard negative)
- SecuritySensitive (40 pairs: 15 equivalent, 25 hard negative)
- SystemAdministrative (40 pairs: 15 equivalent, 25 hard negative)
- LegalCompliance (40 pairs: 15 equivalent, 25 hard negative)
- DevOpsCloud (40 pairs: 15 equivalent, 25 hard negative)
- SoftwareEngineering (40 pairs: 15 equivalent, 25 hard negative)
- GeneralReasoning (40 pairs: 15 equivalent, 25 hard negative)
Total: 320 pairs (120 Equivalent, 200 Hard Negative).
"""

import json
import os
import math

def generate_pairs():
    pairs = []
    pid = 1

    # 1. FinancialAccounting (40 pairs)
    fin_pairs = [
        # Equivalent
        ("Retrieve the quarterly cash flow statement for Acme Corp.", "Fetch the quarterly cash flow statement for Acme Corp.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("What is the total operational expense reported by Beta LLC?", "Show the total operational expense reported by Beta LLC.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Calculate the gross revenue for portfolio Alpha in Q1.", "Determine the gross revenue for portfolio Alpha in Q1.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("List all accounts receivable exceeding 10000 dollars.", "Show all accounts receivable exceeding 10000 dollars.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit ledger entries for reconciliation discrepancies.", "Examine ledger entries for reconciliation discrepancies.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display the balance sheet assets for Delta Holdings.", "Show the balance sheet assets for Delta Holdings.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Summarize the annual depreciation schedule for equipment.", "Describe the annual depreciation schedule for equipment.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify wire payment confirmation for invoice 9021.", "Check wire payment confirmation for invoice 9021.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Report treasury liquidity reserves for capital requirements.", "Show treasury liquidity reserves for capital requirements.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Extract EBITDA metrics across European subsidiary entities.", "List EBITDA metrics across European subsidiary entities.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Confirm tax liability estimation for fiscal cycle 2025.", "Verify tax liability estimation for fiscal cycle 2025.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Provide net operating profit margin for division Omega.", "Show net operating profit margin for division Omega.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Review vendor disbursement logs for payment batch 504.", "Audit vendor disbursement logs for payment batch 504.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Analyze currency hedging portfolio against foreign exchange risk.", "Evaluate currency hedging portfolio against foreign exchange risk.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Fetch the general ledger journal entry 7712.", "Retrieve the general ledger journal entry 7712.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Wire 50000 dollars from treasury to vendor Apex.", "Wire 500000 dollars from treasury to vendor Apex.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "numeric_magnitude_shift"),
        ("Approve wire disbursement of 25000 dollars to account 4401.", "Deny wire disbursement of 25000 dollars to account 4401.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "authorization_inversion"),
        ("Transfer 100000 dollars from account 8812 to account 9934.", "Transfer 100000 dollars from account 9934 to account 8812.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "role_numeric_transposition"),
        ("Confirm audit clearance for fiscal year 2024.", "Confirm audit clearance for fiscal year 2025.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "temporal_year_shift"),
        ("Release payment batch 108 to supplier Delta.", "Do not release payment batch 108 to supplier Delta.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "negation_inversion"),
        ("The quarterly earnings statement indicates a profitable quarter.", "The quarterly earnings statement indicates an unprofitable quarter.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "antonym_inversion"),
        ("All offshore accounts are subject to mandatory tax reporting.", "Not all offshore accounts are subject to mandatory tax reporting.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Verify invoice 4022 for payment of 1500 euros.", "Verify invoice 4022 for payment of 1500 pounds.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "currency_mutation"),
        ("Credit account 7710 with dividend distribution of 3200 dollars.", "Debit account 7710 with dividend distribution of 3200 dollars.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "transaction_type_inversion"),
        ("Generate profit report for Q2 operating performance.", "Generate profit report for Q3 operating performance.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "temporal_quarter_shift"),
        ("Authorize escrow release for merger transaction Alpha.", "Block escrow release for merger transaction Alpha.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "authorization_denial"),
        ("Record interest accrued of 450 dollars on treasury note.", "Record interest accrued of 4500 dollars on treasury note.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "numeric_magnitude_shift"),
        ("Allocate 75000 dollars capital expenditure to robotics division.", "Allocate 75000 dollars capital expenditure to logistics division.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "entity_substitution"),
        ("Confirm that customer billing records remain encrypted in storage.", "Confirm that customer billing records remain unencrypted in storage.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "security_negation_flip"),
        ("Disburse 12000 dollars bonus compensation to employee 891.", "Disburse 12000 dollars bonus compensation to employee 892.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "entity_numeric_shift"),
        ("Settle trade execution on exchange Nasdaq for ticker MSFT.", "Settle trade execution on exchange Nasdaq for ticker AAPL.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "ticker_substitution"),
        ("Apply late penalty fee of 150 dollars to overdue invoice.", "Waive late penalty fee of 150 dollars to overdue invoice.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "action_inversion"),
        ("Authorize credit line expansion to 2000000 dollars.", "Reject credit line expansion to 2000000 dollars.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "decision_reversal"),
        ("Audit report verified zero material compliance defects.", "Audit report identified unapproved material compliance defects.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "finding_inversion"),
        ("Transfer funds to beneficiary in London branch.", "Transfer funds to beneficiary in Zurich branch.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "jurisdiction_shift"),
        ("Approve vendor expense invoice 8820 for payment.", "Flag vendor expense invoice 8820 for fraud investigation.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "fraud_flag_inversion"),
        ("Recognize deferred revenue of 60000 dollars in current fiscal period.", "Defer revenue recognition of 60000 dollars to next fiscal period.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "timing_recognition_shift"),
        ("Confirm loan collateral valuation exceeds 500000 dollars.", "Confirm loan collateral valuation falls below 500000 dollars.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "relational_bound_inversion"),
        ("The financial forecast indicates positive cash flow.", "The financial forecast indicates negative cash flow.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "polarity_inversion"),
        ("Disallow transaction processing for unverified cardholder 4110.", "Allow transaction processing for unverified cardholder 4110.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "permission_flip"),
    ]
    for a, b, lbl, dec, shift in fin_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "FinancialAccounting", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 2. HealthcareMedical (40 pairs)
    med_pairs = [
        # Equivalent
        ("Administer 50mg of Atenolol daily for hypertension.", "Provide 50mg of Atenolol once a day for high blood pressure.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Review cardiac ultrasound imaging for left ventricular ejection fraction.", "Examine cardiac ultrasound imaging for left ventricular ejection fraction.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Check serum potassium concentration from blood test panel.", "Evaluate serum potassium concentration from blood test panel.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve patient immunization history for tetanus vaccination.", "Fetch patient immunization history for tetanus vaccination.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Analyze diagnostic biopsy specimens for cellular dysplasia.", "Examine diagnostic biopsy specimens for cellular dysplasia.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Summarize clinical recommendations for diabetic peripheral neuropathy.", "Outline clinical recommendations for diabetic peripheral neuropathy.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify insulin dosage titration protocol for type 2 diabetes.", "Check insulin dosage titration protocol for type 2 diabetes.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display post-operative recovery guidelines for knee arthroplasty.", "Show post-operative recovery guidelines for knee arthroplasty.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Assess renal glomerular filtration rate from serum creatinine.", "Measure renal glomerular filtration rate from serum creatinine.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("List contraindications for prescribing cephalosporin antibiotics.", "Show contraindications for prescribing cephalosporin antibiotics.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Monitor vital signs telemetry for intensive care bed 12.", "Observe vital signs telemetry for intensive care bed 12.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve clinical trial efficacy results for oncology compound TX-400.", "Fetch clinical trial efficacy results for oncology compound TX-400.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Review medication reconciliation record for patient admission 7102.", "Check medication reconciliation record for patient admission 7102.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain emergency treatment pathway for acute myocardial infarction.", "Describe emergency treatment pathway for acute myocardial infarction.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect continuous glucose monitoring telemetry logs.", "Review continuous glucose monitoring telemetry logs.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Administer 5mg dosage of pediatric morphine.", "Administer 50mg dosage of pediatric morphine.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "numeric_magnitude_shift"),
        ("Patient diagnosis confirmed with acute hepatitis antibodies.", "Patient diagnosis unconfirmed with acute hepatitis antibodies.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "antonym_inversion"),
        ("Administer oral anticoagulant warfarin 2mg to patient.", "Do not administer oral anticoagulant warfarin 2mg to patient.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "negation_inversion"),
        ("Prescribe amoxicillin 500mg for streptococcal bacterial pharyngitis in Patient Alpha.", "Prescribe amoxicillin 500mg for streptococcal bacterial pharyngitis in Patient Beta.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "patient_entity_shift"),
        ("Patient reports no documented allergy to penicillin derivatives.", "Patient reports documented severe allergy to penicillin derivatives.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "allergy_negation_inversion"),
        ("Infuse intravenous saline at 100ml per hour.", "Infuse intravenous saline at 1000ml per hour.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "infusion_rate_magnitude"),
        ("Discontinue corticosteroid therapy immediately due to adverse reaction.", "Continue corticosteroid therapy immediately due to adverse reaction.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "therapy_continuation_flip"),
        ("Administer insulin lispro 10 units before evening meal.", "Administer insulin lispro 10 units before morning breakfast.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "temporal_meal_shift"),
        ("Patient blood pressure reading is 120 over 80 mmHg.", "Patient blood pressure reading is 180 over 120 mmHg.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "vitals_numeric_transposition"),
        ("Confirm safe administration of drug A with drug B.", "Contraindicated lethal interaction between drug A and drug B.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "interaction_safety_reversal"),
        ("All intensive care telemetry sensors are properly calibrated.", "Not all intensive care telemetry sensors are properly calibrated.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Patient test result for influenza antigen was positive.", "Patient test result for influenza antigen was negative.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "diagnostic_polarity_flip"),
        ("Administer oral chemotherapy tablet once weekly on monday.", "Administer oral chemotherapy tablet once weekly on friday.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "temporal_day_shift"),
        ("Discharge patient 409 from recovery ward today.", "Do not discharge patient 409 from recovery ward today.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "discharge_negation_flip"),
        ("Inject epinephrine 0.3mg intramuscularly for anaphylaxis.", "Inject epinephrine 3.0mg intramuscularly for anaphylaxis.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "emergency_dosage_magnitude"),
        ("Patient oxygen saturation measured at 98 percent on room air.", "Patient oxygen saturation measured at 84 percent on room air.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "oxygenation_critical_drop"),
        ("Biopsy histology revealed verified benign adenoma tissue.", "Biopsy histology revealed unverified benign adenoma tissue.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "histology_verification_flip"),
        ("Prepare patient for surgical resection of left kidney.", "Prepare patient for surgical resection of right kidney.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "anatomical_laterality_inversion"),
        ("Administer pediatric tylenol suspension 160mg orally.", "Administer adult aspirin tablets 325mg orally.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "pediatric_contraindication_switch"),
        ("Confirm patient signed informed consent for surgical procedure.", "Confirm patient revoked informed consent for surgical procedure.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "consent_revocation_flip"),
        ("Infuse blood unit packed red cells type O negative.", "Infuse blood unit packed red cells type AB positive.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "blood_type_incompatibility"),
        ("Serum troponin test confirmed myocardial injury was prevented.", "Serum troponin test confirmed myocardial injury was not prevented.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "biomarker_interpretation_flip"),
        ("Patient exhibits ventricular arrhythmia at rate of 75 bpm.", "Patient exhibits ventricular arrhythmia at rate of 175 bpm.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cardiac_rate_numeric_shift"),
        ("Increase ventilator positive end-expiratory pressure to 8 cmH2O.", "Decrease ventilator positive end-expiratory pressure to 4 cmH2O.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "ventilator_titration_inversion"),
        ("Prescribe topical hydrocortisone ointment 1 percent twice daily.", "Prescribe systemic prednisone tablets 40mg twice daily.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "administration_route_substitution"),
    ]
    for a, b, lbl, dec, shift in med_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "HealthcareMedical", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 3. SecuritySensitive (40 pairs)
    sec_pairs = [
        # Equivalent
        ("Grant read-only access to engineering telemetry for user dev_92.", "Provide user dev_92 with read permissions on engineering telemetry.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify cryptographic signature on artifact build 204.", "Validate cryptographic signature on artifact build 204.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit failed authentication attempts recorded in auth service.", "Review failed authentication attempts recorded in auth service.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("List active IAM service accounts in production project.", "Show active IAM service accounts in production project.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect TLS cipher suites enabled on edge load balancer.", "Check TLS cipher suites enabled on edge load balancer.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve audit trail for credential generation in tenant T1.", "Fetch audit trail for credential generation in tenant T1.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify HMAC digest for incoming webhook event 881.", "Validate HMAC digest for incoming webhook event 881.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect firewall egress security rules for private subnet.", "Review firewall egress security rules for private subnet.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display active session tokens for user admin_01.", "Show active session tokens for user admin_01.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Check rate limiting quota status for client API key.", "Verify rate limiting quota status for client API key.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("List revoked certificates stored in revocation list 44.", "Show revoked certificates stored in revocation list 44.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Analyze security vulnerability scan results for container base image.", "Examine security vulnerability scan results for container base image.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify zero-knowledge cryptographic proof for identity assertion.", "Validate zero-knowledge cryptographic proof for identity assertion.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display authorized public keys registered for bastion host.", "Show authorized public keys registered for bastion host.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Fetch role-based access permissions assigned to role security_auditor.", "Retrieve role-based access permissions assigned to role security_auditor.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Grant read-only access to engineering telemetry for user dev_92.", "Deny read-only access to engineering telemetry for user dev_92.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "authorization_denial"),
        ("Delete all expired session tokens from redis.", "Do not delete expired session tokens from redis.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "negation_inversion"),
        ("Allow anonymous external traffic to administrative metrics port 9090.", "Disallow anonymous external traffic to administrative metrics port 9090.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "antonym_inversion"),
        ("Rotate master encryption key KEK version 1 to version 2.", "Rotate master encryption key KEK version 1 to version 3.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "version_numeric_shift"),
        ("Permit SSH access from corporate VPN gateway 10.0.0.1.", "Prohibit SSH access from corporate VPN gateway 10.0.0.1.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "permission_prohibition_flip"),
        ("Confirm authentication token validity for user 4410.", "Revoke authentication token validity for user 4410.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "token_revocation_flip"),
        ("Enable mutual TLS client certificate verification on ingress.", "Disable mutual TLS client certificate verification on ingress.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "tls_enforcement_inversion"),
        ("All tenant data buckets are protected by customer-managed KMS keys.", "Not all tenant data buckets are protected by customer-managed KMS keys.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Allow developer service account to assume role production_deployer.", "Block developer service account from assuming role production_deployer.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "role_assumption_block"),
        ("Enforce mandatory multi-factor authentication on administrative login.", "Bypass mandatory multi-factor authentication on administrative login.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "mfa_bypass_inversion"),
        ("Authorize cryptographic shredding of tenant T1 data keys.", "Cancel cryptographic shredding of tenant T1 data keys.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "shred_cancellation"),
        ("Verify SHA-256 hash checksum matches release binary.", "Detected checksum mismatch indicating tampered release binary.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "integrity_check_failure"),
        ("Allow outbound TCP traffic on port 443 to external API.", "Allow outbound TCP traffic on port 22 to external API.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "port_number_shift"),
        ("Authenticate incoming request using HMAC signature validation.", "Accept unverified request without HMAC signature validation.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "signature_validation_bypass"),
        ("Set session token expiration timeout to 3600 seconds.", "Set session token expiration timeout to 36000 seconds.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "timeout_magnitude_expansion"),
        ("Lock user account 810 after 5 consecutive failed attempts.", "Unlock user account 810 after 5 consecutive failed attempts.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "account_lock_inversion"),
        ("Permit read access to encrypted database tables.", "Permit read access to plaintext unencrypted database tables.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "encryption_state_mutation"),
        ("Restrict API key usage to verified IP address 192.168.1.50.", "Allow API key usage from unrestricted public IP range.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "ip_restriction_removal"),
        ("Promote user account 202 to cluster administrator role.", "Demote user account 202 from cluster administrator role.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "privilege_elevation_reversal"),
        ("Reject unauthenticated RPC invocations from untrusted hosts.", "Approve unauthenticated RPC invocations from untrusted hosts.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "access_gate_reversal"),
        ("Verify authorization token signature using Ed25519 public key.", "Accept forged authorization token with forged signature.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cryptographic_forgery_trap"),
        ("Quarantine compromised worker node 15 from production fleet.", "Return compromised worker node 15 to production fleet.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quarantine_release_hazard"),
        ("Enforce strict CORS origin whitelist on authentication handler.", "Disable CORS origin check on authentication handler.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cors_protection_removal"),
        ("Mask customer personal identifying information in application logs.", "Expose raw unmasked customer personal identifying information in application logs.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "pii_exposure_flip"),
        ("Confirm zero vulnerabilities found in static code analysis.", "Static code analysis detected critical remote code execution vulnerability.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "vulnerability_presence_flip"),
    ]
    for a, b, lbl, dec, shift in sec_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "SecuritySensitive", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 4. SystemAdministrative (40 pairs)
    sys_pairs = [
        # Equivalent
        ("Restart cluster node 4 after draining incoming traffic.", "After draining incoming traffic, perform a restart of cluster node 4.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Check disk storage utilization on database shard 2.", "Inspect disk storage utilization on database shard 2.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve CPU load average metrics across worker pool Alpha.", "Fetch CPU load average metrics across worker pool Alpha.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display active connections pool count for Postgres primary.", "Show active connections pool count for Postgres primary.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit system kernel logs for hardware memory parity errors.", "Review system kernel logs for hardware memory parity errors.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify network interface throughput on gateway edge router.", "Check network interface throughput on gateway edge router.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Monitor thread pool queue saturation on backend service.", "Observe thread pool queue saturation on backend service.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("List mounted storage volumes attached to compute cluster 9.", "Show mounted storage volumes attached to compute cluster 9.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect Redis memory fragmentation ratio on replica 1.", "Review Redis memory fragmentation ratio on replica 1.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Check NTP server synchronization clock offset on hosts.", "Verify NTP server synchronization clock offset on hosts.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display cluster ingress routing rules for domain api.internal.", "Show cluster ingress routing rules for domain api.internal.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve garbage collection pause times for runtime JVM.", "Fetch garbage collection pause times for runtime JVM.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit cron scheduled job executions for backup tasks.", "Review cron scheduled job executions for backup tasks.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify DNS resolution latency across regional resolver nodes.", "Check DNS resolution latency across regional resolver nodes.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display system uptime and load status for node worker_08.", "Show system uptime and load status for node worker_08.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Restart cluster node 4 after draining incoming traffic.", "Restart cluster node 14 after draining incoming traffic.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "entity_numeric_shift"),
        ("Scale deployment replicas from 5 instances to 10 instances.", "Scale deployment replicas from 10 instances to 5 instances.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "scaling_direction_inversion"),
        ("Execute database failover promotion on replica db_02.", "Do not execute database failover promotion on replica db_02.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "negation_inversion"),
        ("Purge temporary log files from cluster volume 101.", "Purge temporary log files from cluster volume 201.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "volume_numeric_shift"),
        ("Set TCP keepalive idle timeout to 60 seconds.", "Set TCP keepalive idle timeout to 600 seconds.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "numeric_magnitude_shift"),
        ("Promote staging worker node to production traffic cluster.", "Demote production worker node to staging traffic cluster.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "environment_promotion_flip"),
        ("Drain and terminate compute instance worker_55 immediately.", "Cancel draining and preserve compute instance worker_55.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "lifecycle_action_cancellation"),
        ("All cluster nodes cannot be rebooted simultaneously.", "Not all cluster nodes can be rebooted simultaneously.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Allocate 16 gigabytes of RAM to database buffer pool.", "Allocate 64 gigabytes of RAM to database buffer pool.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "resource_quota_shift"),
        ("Switch primary DNS from server 10 to server 20.", "Switch primary DNS from server 20 to server 10.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "dns_target_transposition"),
        ("Enable asynchronous disk write buffer caching on storage array.", "Disable asynchronous disk write buffer caching on storage array.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cache_mode_inversion"),
        ("Migrate tenant storage volume to region us-east-1.", "Migrate tenant storage volume to region eu-central-1.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "geographic_region_shift"),
        ("Halt background compaction job on Cassandra keyspace.", "Resume background compaction job on Cassandra keyspace.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "job_state_reversal"),
        ("Allow unlimited container memory growth without cgroup limits.", "Enforce strict container memory bounds under cgroup limits.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cgroup_enforcement_flip"),
        ("Drop secondary read index from table order_history.", "Rebuild secondary read index on table order_history.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "index_action_inversion"),
        ("Upgrade system kernel packages to version 6.5.0.", "Upgrade system kernel packages to version 6.6.0.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "kernel_version_shift"),
        ("Configure network MTU size to 1500 bytes on ethernet interface.", "Configure network MTU size to 9000 bytes on ethernet interface.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "jumbo_frame_numeric_shift"),
        ("Rebalance partitioned Kafka topics across 8 brokers.", "Rebalance partitioned Kafka topics across 4 brokers.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "broker_count_mutation"),
        ("Flush all in-memory dirty pages to durable NVMe storage.", "Discard all in-memory dirty pages without flushing to storage.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "data_loss_hazard_reversal"),
        ("Enable automatic crash kernel dump collection on panic.", "Disable automatic crash kernel dump collection on panic.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "kdump_toggle_inversion"),
        ("Migrate virtual machine host from hypervisor 01 to hypervisor 02.", "Migrate virtual machine host from hypervisor 02 to hypervisor 01.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "hypervisor_direction_flip"),
        ("Set database lock timeout to 5000 milliseconds.", "Set database lock timeout to 50000 milliseconds.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "lock_timeout_magnitude"),
        ("Authorize rolling restart of production Elasticsearch cluster.", "Abort rolling restart of production Elasticsearch cluster.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cluster_op_abort"),
        ("Confirm network BGP route advertisement is active.", "Confirm network BGP route advertisement is withdrawn.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "bgp_state_inversion"),
        ("Apply system configuration patch to staging environment.", "Apply system configuration patch to production environment.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "environment_target_shift"),
    ]
    for a, b, lbl, dec, shift in sys_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "SystemAdministrative", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 5. LegalCompliance (40 pairs)
    leg_pairs = [
        # Equivalent
        ("Retrieve GDPR data processing agreement signed with vendor X.", "Fetch GDPR data processing agreement signed with vendor X.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Summarize intellectual property assignment clauses in contract 102.", "Outline intellectual property assignment clauses in contract 102.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Review non-disclosure agreement expiration dates for partner Apex.", "Check non-disclosure agreement expiration dates for partner Apex.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit consumer privacy consent records for marketing telemetry.", "Examine consumer privacy consent records for marketing telemetry.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display dispute resolution and arbitration terms under agreement.", "Show dispute resolution and arbitration terms under agreement.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify compliance status against HIPAA privacy security rule.", "Check compliance status against HIPAA privacy security rule.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve customer data export logs under CCPA regulations.", "Fetch customer data export logs under CCPA regulations.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Summarize indemnification limits specified in master service agreement.", "Describe indemnification limits specified in master service agreement.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect third-party vendor risk assessment questionnaires.", "Review third-party vendor risk assessment questionnaires.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display employee compliance training completion records for 2024.", "Show employee compliance training completion records for 2024.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Check records retention policy requirements for tax documents.", "Verify records retention policy requirements for tax documents.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve export control classification numbers for cryptography software.", "Fetch export control classification numbers for cryptography software.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit whistle-blower complaint reports logged in compliance portal.", "Review whistle-blower complaint reports logged in compliance portal.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify license compliance audit for open source libraries.", "Check license compliance audit for open source libraries.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display privacy impact assessment report for project Gemini.", "Show privacy impact assessment report for project Gemini.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Customer provided explicit consent for third-party marketing sharing.", "Customer refused explicit consent for third-party marketing sharing.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "consent_refusal_flip"),
        ("Execute GDPR Article 17 right to erasure for user 9912.", "Do not execute GDPR Article 17 right to erasure for user 9912.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "erasure_negation_flip"),
        ("Indemnification liability is capped at 1000000 dollars.", "Indemnification liability is capped at 10000000 dollars.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "liability_cap_magnitude"),
        ("Governing law and jurisdiction is Delaware court.", "Governing law and jurisdiction is London court.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "jurisdiction_forum_shift"),
        ("The agreement allows assignment of rights to subsidiaries.", "The agreement prohibits assignment of rights to subsidiaries.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "assignment_prohibition_flip"),
        ("All contracting parties are not released from liability obligations.", "Not all contracting parties are released from liability obligations.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Customer signed data processing addendum version 1.0.", "Customer signed data processing addendum version 2.0.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "addendum_version_shift"),
        ("Terminate commercial distribution license with 30 days notice.", "Terminate commercial distribution license with 90 days notice.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "notice_period_numeric_shift"),
        ("Vendor represents open source GPL contamination was prevented.", "Vendor represents open source GPL contamination was not prevented.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "license_contamination_reversal"),
        ("Authorize disclosure of confidential trade secret information.", "Prevent disclosure of confidential trade secret information.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "trade_secret_disclosure_block"),
        ("Confirm non-solicitation clause duration is 12 months.", "Confirm non-solicitation clause duration is 24 months.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "duration_numeric_shift"),
        ("Allow employee ownership of patent inventions created on personal time.", "Assign company ownership of patent inventions created on personal time.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "patent_ownership_reversal"),
        ("Customer data will be processed strictly inside territory of France.", "Customer data will be processed strictly inside territory of Germany.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "data_residency_inversion"),
        ("Warranty disclaimer excludes consequential and punitive damages.", "Warranty disclaimer includes consequential and punitive damages.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "damages_exclusion_flip"),
        ("Approve settlement agreement resolving patent infringement lawsuit.", "Reject settlement agreement resolving patent infringement lawsuit.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "settlement_rejection"),
        ("Contract automatically renews for additional term of 1 year.", "Contract automatically terminates without renewal after term of 1 year.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "renewal_termination_inversion"),
        ("Disclose material regulatory investigation in annual 10-K report.", "Conceal material regulatory investigation in annual 10-K report.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "disclosure_concealment_flip"),
        ("Confirm subcontractor possesses ISO 27001 security certification.", "Confirm subcontractor lacks ISO 27001 security certification.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "certification_status_inversion"),
        ("Penalty for early contract termination is 50000 dollars.", "Penalty for early contract termination is 500000 dollars.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "penalty_magnitude_shift"),
        ("Allow unilateral modification of terms of service without prior notice.", "Require mutual bilateral consent for modification of terms of service.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "unilateral_bilateral_switch"),
        ("Verify retention of biometric records for 30 days.", "Verify retention of biometric records for 365 days.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "retention_duration_shift"),
        ("The agreement grants exclusive commercial licensing rights.", "The agreement grants non-exclusive commercial licensing rights.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "exclusivity_clause_inversion"),
        ("Permit cross-border data transfer without standard contractual clauses.", "Prohibit cross-border data transfer without standard contractual clauses.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cross_border_prohibition"),
        ("Confirm company complied with SEC whistleblower protection statutes.", "Confirm company violated SEC whistleblower protection statutes.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "statutory_compliance_reversal"),
        ("Release escrow source code deposit upon vendor bankruptcy filing.", "Withhold escrow source code deposit upon vendor bankruptcy filing.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "escrow_release_block"),
    ]
    for a, b, lbl, dec, shift in leg_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "LegalCompliance", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 6. DevOpsCloud (40 pairs)
    dev_pairs = [
        # Equivalent
        ("Deploy container image to Kubernetes cluster Alpha.", "Roll out container image to Kubernetes cluster Alpha.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect Terraform state file locks in cloud storage bucket.", "Check Terraform state file locks in cloud storage bucket.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve build artifact digests from Docker registry.", "Fetch build artifact digests from Docker registry.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Monitor cloud autoscaling group instance capacity.", "Observe cloud autoscaling group instance capacity.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify VPC peering routing table entries between subnets.", "Check VPC peering routing table entries between subnets.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display active cloud load balancer listener configurations.", "Show active cloud load balancer listener configurations.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect CloudWatch alarm thresholds for error rate alerting.", "Review CloudWatch alarm thresholds for error rate alerting.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve Jenkins CI/CD pipeline build duration history.", "Fetch Jenkins CI/CD pipeline build duration history.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Audit cloud storage bucket public access block settings.", "Review cloud storage bucket public access block settings.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify DNS health check probe status for endpoint api.app.com.", "Check DNS health check probe status for endpoint api.app.com.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display container restart count across namespace production.", "Show container restart count across namespace production.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect AWS CloudFormation stack resource creation events.", "Review AWS CloudFormation stack resource creation events.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve Prometheus scrape target status for service gateway.", "Fetch Prometheus scrape target status for service gateway.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify Istio service mesh mutual TLS configuration.", "Check Istio service mesh mutual TLS configuration.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display cloud budget spend velocity alerts for current billing period.", "Show cloud budget spend velocity alerts for current billing period.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Deploy container image version 1.4.0 to production.", "Deploy container image version 1.4.1 to production.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "container_version_shift"),
        ("Execute terraform destroy on production infrastructure.", "Do not execute terraform destroy on production infrastructure.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "negation_inversion"),
        ("Configure storage bucket acl to private access for Project Alpha.", "Configure storage bucket acl to public access for Project Beta.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "bucket_permission_exposure"),
        ("Scale Kubernetes deployment from 3 pods to 30 pods.", "Scale Kubernetes deployment from 3 pods to 3 pods.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "pod_scale_mutation"),
        ("Set container CPU limit to 500 millicores.", "Set container CPU limit to 5000 millicores.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cpu_limit_magnitude"),
        ("Route ingress traffic to primary availability zone ZoneA.", "Route ingress traffic to primary availability zone ZoneB.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "az_target_shift"),
        ("Enable automated database snapshot backup retention for 7 days.", "Disable automated database snapshot backup retention for 7 days.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "backup_retention_disable"),
        ("All cloud infrastructure resources are tagged with department cost center.", "Not all cloud infrastructure resources are tagged with department cost center.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Promote blue container deployment to active green slot.", "Abort blue container deployment and rollback to green slot.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "deployment_promotion_abort"),
        ("Set HTTP request timeout to 15 seconds on API gateway.", "Set HTTP request timeout to 150 seconds on API gateway.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "timeout_numeric_shift"),
        ("Allow ingress traffic from CIDR block 10.0.0.0/16.", "Allow ingress traffic from CIDR block 0.0.0.0/0.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cidr_security_widening"),
        ("Drain nodes in cluster pool staging-west.", "Drain nodes in cluster pool production-west.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cluster_pool_shift"),
        ("Attach IAM managed policy ReadOnlyAccess to developer role.", "Attach IAM managed policy AdministratorAccess to developer role.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "iam_privilege_escalation"),
        ("Configure persistent volume reclaim policy to Retain.", "Configure persistent volume reclaim policy to Delete.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "reclaim_policy_inversion"),
        ("Trigger Canary deployment with 10 percent traffic weighting.", "Trigger Canary deployment with 90 percent traffic weighting.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "canary_weight_shift"),
        ("Enable cloud trail multi-region log file validation.", "Disable cloud trail multi-region log file validation.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "audit_trail_disable"),
        ("Set maximum autoscaling cluster size to 20 nodes.", "Set maximum autoscaling cluster size to 200 nodes.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "asg_limit_shift"),
        ("Assign elastic static IP 54.210.10.4 to web proxy.", "Assign elastic static IP 54.210.10.5 to web proxy.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "ip_address_numeric_shift"),
        ("Configure nginx proxy buffer size to 4 kilobytes.", "Configure nginx proxy buffer size to 64 kilobytes.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "buffer_size_shift"),
        ("Restart 3 Redis cache instances in rolling sequential order.", "Restart 30 Redis cache instances in rolling sequential order.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "concurrency_downtime_inversion"),
        ("Delete unused EBS storage volume vol-08912.", "Delete active production EBS storage volume vol-08912.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "volume_deletion_hazard"),
        ("Enable AWS Shield Advanced distributed denial of service protection.", "Disable AWS Shield Advanced distributed denial of service protection.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "ddos_protection_disable"),
        ("Upgrade Terraform provider aws from 5.10.0 to 5.20.0.", "Downgrade Terraform provider aws from 5.20.0 to 5.10.0.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "upgrade_downgrade_inversion"),
        ("Execute Helm chart release rollback to revision 4.", "Execute Helm chart release rollback to revision 14.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "revision_number_shift"),
        ("Confirm SSL certificate renewal completed successfully.", "Alert SSL certificate renewal failed due to DNS challenge timeout.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "renewal_failure_flip"),
    ]
    for a, b, lbl, dec, shift in dev_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "DevOpsCloud", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 7. SoftwareEngineering (40 pairs)
    swe_pairs = [
        # Equivalent
        ("Refactor asynchronous event loop handler in Rust gateway.", "Restructure asynchronous event loop handler in Rust gateway.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect unit test coverage metrics for database crate.", "Check unit test coverage metrics for database crate.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Review pull request changes for memory leak remediation.", "Examine pull request changes for memory leak remediation.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify binary serialization schema backwards compatibility.", "Check binary serialization schema backwards compatibility.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Analyze compiler profile build timings for release targets.", "Examine compiler profile build timings for release targets.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display benchmark throughput results for exact cache engine.", "Show benchmark throughput results for exact cache engine.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect topological sort implementation for DAG planner.", "Review topological sort implementation for DAG planner.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve open bug reports tagged with priority P0.", "Fetch open bug reports tagged with priority P0.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Check thread synchronization lock contention in mutex pool.", "Verify thread synchronization lock contention in mutex pool.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Summarize design architecture for dual vector-graph gateway.", "Outline design architecture for dual vector-graph gateway.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Inspect OpenAPI schema definitions for authentication endpoints.", "Review OpenAPI schema definitions for authentication endpoints.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify property-based test invariants for Merkle tree proofs.", "Check property-based test invariants for Merkle tree proofs.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Display code linters status across Rust workspace packages.", "Show code linters status across Rust workspace packages.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Retrieve memory allocation profiling heap flamegraph.", "Fetch memory allocation profiling heap flamegraph.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Verify semantic versioning bump for library release 1.2.0.", "Check semantic versioning bump for library release 1.2.0.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Merge pull request 405 into branch main.", "Close and reject pull request 405 without merging.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "pr_decision_reversal"),
        ("Revert database migration 0006 in production database.", "Do not revert database migration 0006 in production database.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "negation_inversion"),
        ("Replace hash function MurmurHash3 with cryptographic SHA-256.", "Replace hash function SHA-256 with non-cryptographic MurmurHash3.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "hash_security_direction_flip"),
        ("Set connection pool maximum capacity to 10 connections.", "Set connection pool maximum capacity to 100 connections.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "pool_capacity_shift"),
        ("Enable thread-safe atomic memory ordering SeqCst.", "Use relaxed unsynchronized memory ordering Relaxed.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "memory_ordering_downgrade"),
        ("All unit tests must pass with zero compilation warnings.", "Not all unit tests must pass with zero compilation warnings.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quantifier_scope_inversion"),
        ("Upgrade dependency tokio from version 1.30.0 to 1.35.0.", "Downgrade dependency tokio from version 1.35.0 to 1.30.0.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "dependency_version_reversal"),
        ("Commit rebase onto branch develop before release tag.", "Force push overwriting commit history on branch main.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "destructive_git_operation"),
        ("Implement zero-copy serialization using 64 byte buffers.", "Implement zero-copy serialization using 6400 byte buffers.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "buffer_size_shift"),
        ("Catch unhandled panic exceptions and return internal error 500.", "Allow thread panic to crash the entire gateway runtime.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "crash_safety_inversion"),
        ("Set async timeout deadline to 200 milliseconds.", "Set async timeout deadline to 2000 milliseconds.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "deadline_magnitude_shift"),
        ("Verify that function foo() is pure and idempotent.", "Function foo() performs mutating side effects on disk.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "idempotency_violation_flip"),
        ("Execute fuzzing campaign for 1000000 iterations.", "Execute fuzzing campaign for 10000 iterations.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "fuzz_scale_shift"),
        ("Enable SIMD vectorization optimizations on x86 AVX2 target.", "Disable SIMD vectorization fallback to scalar emulation.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "simd_optimization_disable"),
        ("Allocate memory buffer of 64 megabytes for incoming payload.", "Allocate memory buffer of 640 megabytes for incoming payload.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "buffer_size_explosion"),
        ("Confirm that method returns Result<T, Error> on failure.", "Method invokes std::process::exit(1) on failure.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "error_handling_divergence"),
        ("Validate JSON schema against OpenAPI specification version 3.0.", "Validate JSON schema against OpenAPI specification version 3.1.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "spec_version_shift"),
        ("Configure bounded mpsc channel capacity to 1000 messages.", "Configure unbounded mpsc channel capacity risking out of memory.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "bounded_unbounded_switch"),
        ("Inline critical hot path function for zero function call overhead.", "Disable inlining forcing dynamic virtual dispatch overhead.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "inlining_dispatch_reversal"),
        ("Enforce compile-time lifetimes avoiding use-after-free defects.", "Use unsafe pointer dereference causing undefined behavior.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "memory_safety_violation"),
        ("Confirm integration test suite exited with code 0.", "Integration test suite failed with exit code 1.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "exit_code_failure_flip"),
        ("Serialize data structures to standard JSON format.", "Serialize data structures to raw unescaped binary format.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "format_encoding_shift"),
        ("Initialize cryptographic random number generator with secure OS entropy.", "Seed random number generator with constant hardcoded zero seed.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "entropy_seed_weakness"),
        ("Bind TCP server socket to localhost interface 127.0.0.1.", "Bind TCP server socket to public wildcard interface 0.0.0.0.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "network_binding_exposure"),
        ("Verify that memory buffer allocation does not overflow 32-bit limit.", "Allocate memory buffer exceeding 32-bit integer boundaries.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "integer_overflow_hazard"),
    ]
    for a, b, lbl, dec, shift in swe_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "SoftwareEngineering", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    # 8. GeneralReasoning (40 pairs)
    gen_pairs = [
        # Equivalent
        ("Summarize the architectural differences between TCP and UDP.", "Provide an overview comparing TCP protocol versus UDP protocol.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain the consensus mechanism utilized by Raft algorithm.", "Describe the consensus mechanism utilized by Raft algorithm.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Analyze the computational complexity of quicksort in average case.", "Evaluate the computational complexity of quicksort in average case.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Compare relational database ACID guarantees with eventual consistency.", "Contrast relational database ACID guarantees with eventual consistency.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain how consistent hashing minimizes cache reshuffling.", "Describe how consistent hashing minimizes cache reshuffling.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Outline the fundamental principles of zero trust network architecture.", "Summarize the fundamental principles of zero trust network architecture.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain the mathematical foundations of public key cryptography.", "Describe the mathematical foundations of public key cryptography.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Compare optimistic concurrency control with two-phase locking.", "Contrast optimistic concurrency control with two-phase locking.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain the purpose of Bloom filters in database query optimization.", "Describe the purpose of Bloom filters in database query optimization.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Summarize the CAP theorem trade-offs between consistency and availability.", "Describe the CAP theorem trade-offs between consistency and availability.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain how vector embeddings represent semantic meaning in NLP.", "Describe how vector embeddings represent semantic meaning in NLP.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Compare message queuing patterns point-to-point versus publish-subscribe.", "Contrast message queuing patterns point-to-point versus publish-subscribe.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain the difference between synchronous and asynchronous I/O models.", "Describe the difference between synchronous and asynchronous I/O models.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Outline the core mechanics of transformer attention self-attention layers.", "Summarize the core mechanics of transformer attention self-attention layers.", "EQUIVALENT", "REUSE_SAFE", "none"),
        ("Explain how write-ahead logging guarantees durability in databases.", "Describe how write-ahead logging guarantees durability in databases.", "EQUIVALENT", "REUSE_SAFE", "none"),
        # Hard Negative
        ("Explain how TCP guarantees reliable in-order packet delivery.", "Explain how UDP provides unreliable connectionless packet transmission.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "protocol_inversion"),
        ("A binary search tree maintains sorted order in logarithmic time for Dataset Alpha.", "A hash map maintains un-ordered keys in constant time for Dataset Beta.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "data_structure_divergence"),
        ("Explain why Paxos algorithm guarantees consensus under network partitions.", "Explain why Paxos algorithm fails consensus under Byzantine adversarial faults.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "consensus_fault_model_flip"),
        ("Explain why asymmetric encryption latency is 5 milliseconds under algorithm RSA.", "Explain why asymmetric encryption latency is 50 milliseconds under algorithm RSA.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "latency_numeric_shift"),
        ("Describe why concurrency model Alpha prevents lock stalls.", "Describe why concurrency model Beta prevents lock stalls.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "concurrency_model_shift"),
        ("All distributed nodes can reach consensus in zero network hops.", "No distributed nodes can reach consensus in zero network hops.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "possibility_quantifier_negation"),
        ("Calculate speedup factor under Amdahl law for 4 parallel processors.", "Calculate speedup factor under Amdahl law for 64 parallel processors.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "processor_count_shift"),
        ("Explain how B-tree indexes optimize disk block seek operations.", "Explain how LSM trees optimize sequential append write operations.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "storage_engine_contrast"),
        ("Describe how digital certificates prevent forged TLS sessions.", "Describe how digital certificates allow forged TLS sessions.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "forgery_allowance_inversion"),
        ("Explain why quicksort worst case running time is O(N^2).", "Explain why merge sort worst case running time is O(N log N).", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "complexity_assertion_mismatch"),
        ("Explain why connection pool size of 10 reduces handshake latency.", "Explain why connection pool size of 100 reduces handshake latency.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "pool_size_numeric_shift"),
        ("Explain the role of write barrier instructions in memory consistency models.", "Explain how instruction reordering breaks sequential consistency without memory barriers.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "hardware_memory_inversion"),
        ("Describe why raft leader election requires majority quorum greater than 50 percent.", "Explain why raft split brain occurs if quorum is set to less than 50 percent.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "quorum_condition_flip"),
        ("Explain how CPU L1 cache line invalidation causes false sharing.", "Explain how cache line padding eliminates false sharing in concurrent code.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "architecture_optimization_flip"),
        ("Describe why dynamic programming memoizes recursive overlapping subproblems.", "Explain why greedy algorithms make locally optimal choices without backtracking.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "algorithmic_paradigm_contrast"),
        ("Explain the scheduling mechanics of scheduler AlgorithmA.", "Explain the scheduling mechanics of scheduler AlgorithmB.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "scheduler_algorithm_shift"),
        ("Describe how garbage collection mark-and-sweep identifies unreachable cyclic objects.", "Explain why reference counting collectors fail to reclaim circular object graphs.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "gc_mechanism_limitation_flip"),
        ("Explain why system architecture ModelOne prioritizes modularity.", "Explain why system architecture ModelTwo prioritizes modularity.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "architecture_model_shift"),
        ("Describe how zero-knowledge proofs verify knowledge without revealing secrets.", "Explain how plaintext password transmission reveals authentication credentials.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "cryptographic_property_reversal"),
        ("Explain why two-phase commit is a blocking atomic commit protocol.", "Explain why three-phase commit is a non-blocking protocol under fail-stop models.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "protocol_blocking_property_flip"),
        ("Describe how SIMD instructions process 8 floating point numbers in single cycle.", "Describe how scalar instructions process single floating point number per cycle.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "instruction_throughput_contrast"),
        ("Explain why hash collisions occur with 32 bit hash keys.", "Explain why hash collisions occur with 64 bit hash keys.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "key_size_numeric_shift"),
        ("Explain how content addressable storage prevents unauthorized mutation.", "Explain how content addressable storage allows unauthorized mutation.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "mutation_authorization_inversion"),
        ("Explain how content addressable storage ModelA guarantees deduplication.", "Explain how content addressable storage ModelB guarantees deduplication.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "storage_model_shift"),
        ("Describe why ACID isolation prevents dirty read anomalies.", "Explain how read uncommitted isolation allows dirty reads.", "HARD_NEGATIVE", "ABSTAIN_OR_RECOMPUTE", "isolation_level_contrast"),
    ]
    for a, b, lbl, dec, shift in gen_pairs:
        pairs.append({"id": f"pair_{pid:03d}", "stratum": "GeneralReasoning", "prompt_a": a, "prompt_b": b, "label": lbl, "expected_decision": dec, "structural_shift": shift})
        pid += 1

    return pairs

def main():
    pairs = generate_pairs()
    print(f"Generated {len(pairs)} pairs across 8 strata.")
    
    eq_count = sum(1 for p in pairs if p["label"] == "EQUIVALENT")
    hn_count = sum(1 for p in pairs if p["label"] == "HARD_NEGATIVE")
    print(f"Equivalent: {eq_count}, Hard Negative: {hn_count}")

    output_path = os.path.join("research", "calibration", "held_out_eval_dataset.json")
    
    dataset = {
        "dataset_metadata": {
            "version": "2.0.0",
            "evaluation_standard": "IEEE-730/ACM-Artifact-Evaluation/ISO-IEC-25010",
            "description": "Pre-registered held-out semantic evaluation benchmark for conformal risk control and AST guardrail validation (N=320 stratified pairs across 8 domains and 13 failure dimensions)",
            "total_samples": len(pairs),
            "domains": [
                "financial_accounting",
                "healthcare_clinical",
                "security_sensitive",
                "system_administrative",
                "legal_compliance",
                "devops_cloud",
                "software_engineering",
                "general_reasoning"
            ],
            "strata_breakdown": {
                "FinancialAccounting": 40,
                "HealthcareMedical": 40,
                "SecuritySensitive": 40,
                "SystemAdministrative": 40,
                "LegalCompliance": 40,
                "DevOpsCloud": 40,
                "SoftwareEngineering": 40,
                "GeneralReasoning": 40
            },
            "statistical_design": {
                "alpha_nominal": 0.001,
                "target_false_positive_rate": 0.0,
                "confidence_level": 0.999,
                "min_sample_size": 300
            }
        },
        "evaluation_strata": pairs
    }

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(dataset, f, indent=2)
    print(f"Successfully wrote {output_path} with {len(pairs)} evaluation pairs.")

if __name__ == "__main__":
    main()
