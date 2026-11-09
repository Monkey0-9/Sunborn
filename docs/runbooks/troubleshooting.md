# Operational Troubleshooting Runbook

This runbook documents diagnostic steps and recovery procedures for common edge cases and runtime alerts encountered in Sunborn deployments.

---

## 1. Database Connection Pool Exhaustion / Outage

### Symptoms
- Gateway logs display: `PostgreSQL connection circuit breaker OPEN; failing fast without acquire`.
- Database write latency spikes or returns 503/500 errors.

### Root Cause Analysis
- Network partition between gateway pods and PostgreSQL primary.
- Connection leak or saturation of the 20-connection pool ceiling.

### Diagnostic & Recovery Steps
1. Verify database process health:
   ```bash
   psql -h <POSTGRES_HOST> -U sunborn -d sunborn -c "SELECT count(*), state FROM pg_stat_activity GROUP BY state;"
   ```
2. Verify gateway readiness probe:
   ```bash
   curl -i http://<GATEWAY_HOST>:8080/ready
   ```
3. If PostgreSQL was restarted, reset the circuit breaker via administrative endpoint:
   ```bash
   curl -X POST http://<GATEWAY_HOST>:8080/v1/chaos/reset \
     -H "Authorization: Bearer $SUNBORN_ADMIN_KEY"
   ```

---

## 2. Invalidation Outbox Queue Backpressure & DLQ Accumulation

### Symptoms
- Invalidation events log: `Invalidation queue backpressure limit reached`.
- Dead letter queue count increases (`dlq_count > 0`).

### Root Cause Analysis
- Sustained high burst of document ingestion exceeding consumption rate.
- Unresponsive downstream cache or vector store node.

### Diagnostic & Recovery Steps
1. Inspect current DLQ depth via metrics or debug endpoint:
   ```bash
   curl -s http://<GATEWAY_HOST>:8080/metrics | grep sunborn_invalidation_dlq
   ```
2. Identify failing document IDs and error reasons from application logs:
   ```bash
   kubectl logs -l app.kubernetes.io/name=sunborn-gateway --tail=100 | grep "Dead Letter"
   ```
3. Trigger automated DLQ re-injection once downstream store recovers:
   ```bash
   curl -X POST http://<GATEWAY_HOST>:8080/v1/lifecycle/sweep \
     -H "Authorization: Bearer $SUNBORN_ADMIN_KEY"
   ```

---

## 3. Remote KMS Provider Handshake Failure

### Symptoms
- In production mode (`SUNBORN_ENV=production`), gateway refuses to boot with error:
  `Production startup strictly rejects simulated local-aead KMS provider`.

### Root Cause Analysis
- Gateway pod lacks IAM role credentials for AWS KMS or HashiCorp Vault transit engine.

### Diagnostic & Recovery Steps
1. In production, ensure `SUNBORN_KMS_PROVIDER` is set to `aws-kms` or `vault`.
2. For AWS KMS:
   ```bash
   aws kms describe-key --key-id $SUNBORN_AWS_KMS_KEY_ID
   ```
3. Verify that the Kubernetes ServiceAccount is annotated with the IAM Role (`eks.amazonaws.com/role-arn`).

---

## 4. Conformal Calibration Drift Alarm

### Symptoms
- Cache hit rate plummets suddenly; logs indicate `Streaming OOD detector triggered conservative fail-safe abstention`.

### Root Cause Analysis
- Significant distribution shift in incoming user prompts compared to offline calibration set.

### Diagnostic & Recovery Steps
1. Inspect online drift scores:
   ```bash
   curl -s http://<GATEWAY_HOST>:8080/v1/autonomic/status \
     -H "Authorization: Bearer $SUNBORN_ADMIN_KEY"
   ```
2. Re-calibrate conformal thresholds by running an updated batch of representative domain traces:
   ```bash
   curl -X POST http://<GATEWAY_HOST>:8080/v1/learning/calibrate \
     -H "Authorization: Bearer $SUNBORN_ADMIN_KEY" \
     -H "Content-Type: application/json" \
     -d '{"sample_size": 250, "target_alpha": 0.05}'
   ```
