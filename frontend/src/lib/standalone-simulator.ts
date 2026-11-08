/**
 * Sunborn High-Fidelity Standalone Simulation Engine
 * Serves mission-critical fallback telemetry and realistic responses
 * when the local Rust Gateway (127.0.0.1:8080) is offline during frontend evaluation.
 */

export function handleSimulatedRoute(
  method: string,
  normalizedPath: string,
  rawBody?: string
): { status: number; body: any; headers?: Record<string, string> } | null {
  const norm = normalizedPath.toLowerCase();

  // 1. Health & Cluster Status
  if (norm === "health" || norm === "ready") {
    return {
      status: 200,
      body: {
        status: "ok",
        mode: "standalone-simulation",
        timestamp: new Date().toISOString(),
        kms_provider: "local-aead",
        cluster_state: "synchronized",
        status_observed_at: new Date().toISOString(),
      },
    };
  }

  if (norm === "metrics") {
    return {
      status: 200,
      body: "# HELP sunborn_requests_total Total HTTP requests\n# TYPE sunborn_requests_total counter\nsunborn_requests_total 12450\n",
      headers: { "Content-Type": "text/plain" },
    };
  }

  // 2. Multi-Region Cluster Status
  if (norm === "v1/cluster/status") {
    return {
      status: 200,
      body: {
        node_id: "gateway-primary-node-1",
        region: "us-east-1",
        cluster_state: "synchronized",
        vector_clock: {
          epoch: 4193,
          "node-us-east-1": 4193,
          "node-eu-central-1": 4189,
          "node-ap-southeast-1": 4184,
        },
        anti_entropy_protocol: "Merkle Tree Differential Gossip",
        consensus_mode: "Authoritative PostgreSQL Watermark",
      },
    };
  }

  // 3. FinOps Economics & Spend Velocity
  if (norm === "v1/finops/report") {
    return {
      status: 200,
      body: {
        metrics: {
          requests_total: 12450,
          total_hits: 9840,
          exact_hits: 6840,
          semantic_hits: 3000,
        },
        economics: {
          total_estimated_savings_usd: 142.5,
          avoided_tokens: 3840000,
          monthly_run_rate_usd: 1240.0,
        },
        spend_velocity: {
          tenant_ceiling_usd: 100.0,
          current_spend_usd: 14.8,
          current_reserved_usd: 2.5,
          burn_rate_hourly_usd: 0.42,
        },
      },
    };
  }

  // 4. Security Audit & WAF Firewall Evaluation
  if (norm === "v1/security/audit") {
    return {
      status: 200,
      body: {
        tenant_id: "tenant-enterprise-fintech",
        events: [
          {
            id: "sec_981",
            event: "ZERO_TRUST_VERIFICATION",
            origin: "Gateway Ingress",
            detail: "Authenticated tenant tenant-enterprise-fintech via SHA-256 key hash",
            status: "200_OK",
            time: "Just now",
          },
          {
            id: "sec_980",
            event: "CONFORMAL_GUARDRAIL_PASS",
            origin: "Decision Engine Tier 2",
            detail: "Query admitted with risk score 0.0004 (alpha <= 0.001)",
            status: "ADMITTED",
            time: "2s ago",
          },
          {
            id: "sec_979",
            event: "CRYPTO_SHRED_ISOLATION",
            origin: "Tenant Key Registry",
            detail: "Tenant key verified active in local AEAD software boundary",
            status: "ACTIVE",
            time: "15s ago",
          },
        ],
      },
    };
  }

  if (norm === "v1/security/evaluate") {
    let parsed: any = {};
    try {
      if (rawBody) parsed = JSON.parse(rawBody);
    } catch {}
    const prompt = (parsed.prompt || "").toLowerCase();
    const isThreat =
      prompt.includes("ignore") ||
      prompt.includes("system prompt") ||
      prompt.includes("bypass") ||
      prompt.includes("password") ||
      prompt.includes("drop table") ||
      prompt.includes("credential");

    return {
      status: 200,
      body: {
        threatDetected: isThreat,
        ruleMatched: isThreat
          ? "OWASP-LLM-01: System Prompt Extraction & Privilege Hijack"
          : "CLEAN_INGRESS",
        action: isThreat
          ? "DROPPED_BEFORE_CACHE_EVALUATION"
          : "ADMITTED_TO_IDENTITY_CANONICALIZER",
        conformalScore: isThreat ? 0.999 : 0.001,
        tenantId: "tenant-enterprise-fintech",
        timestamp: new Date().toISOString(),
      },
    };
  }

  // 5. Chat Completions & Prompt Caching
  if (norm === "v1/chat/completions") {
    let parsed: any = {};
    try {
      if (rawBody) parsed = JSON.parse(rawBody);
    } catch {}
    const messages = parsed.messages || [];
    const lastMsg = messages[messages.length - 1]?.content || "";
    const isExactHit = lastMsg.length > 0 && lastMsg.length % 2 === 0;

    return {
      status: 200,
      body: {
        id: `chatcmpl_sim_${Math.random().toString(36).substring(2, 9)}`,
        object: "chat.completion",
        created: Math.floor(Date.now() / 1000),
        model: parsed.model || "gpt-4o",
        choices: [
          {
            index: 0,
            message: {
              role: "assistant",
              content: isExactHit
                ? "Verified execution reuse from Sunborn DragonflyDB exact tier."
                : "Sunborn multi-tier architecture enforces provable non-accelerator authority, ensuring all stateful invalidations and Merkle proofs are durable in PostgreSQL WAL.",
            },
            finish_reason: "stop",
          },
        ],
        usage: {
          prompt_tokens: Math.max(12, Math.ceil(lastMsg.length / 4)),
          completion_tokens: isExactHit ? 15 : 42,
          total_tokens: Math.max(12, Math.ceil(lastMsg.length / 4)) + (isExactHit ? 15 : 42),
        },
        sunborn_cached: isExactHit,
        sunborn_tier: isExactHit ? "exact" : "origin",
      },
    };
  }

  // 6. Execution DAG & Merkle Attestation
  if (norm === "v1/governance/attest") {
    return {
      status: 200,
      body: {
        root_hash: "7a9b1c3d5e7f8a90123456789abcdef0123456789abcdef0123456789abcdef0",
        proofs: [
          {
            leaf_hash: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
            root_hash: "7a9b1c3d5e7f8a90123456789abcdef0123456789abcdef0123456789abcdef0",
            path: [],
          },
        ],
      },
    };
  }

  if (norm === "v1/governance/verify-proof") {
    return {
      status: 200,
      body: {
        is_valid: true,
        valid: true,
        root_hash: "7a9b1c3d5e7f8a90123456789abcdef0123456789abcdef0123456789abcdef0",
        leaf_hash: "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2",
      },
    };
  }

  // 7. Speculative Markov Prefetching
  if (norm === "v1/speculative/predict") {
    return {
      status: 200,
      body: {
        predictions: [
          {
            predicted_action: "fetch_document_content",
            confidence_score: 0.85,
            estimated_savings_ms: 380,
          },
          {
            predicted_action: "summarize_results",
            confidence_score: 0.15,
            estimated_savings_ms: 190,
          },
        ],
        speculative_cache_warmed: true,
      },
    };
  }

  // 8. Governance: Cryptographic Shredding & State Rebuild
  if (norm === "v1/governance/shred") {
    let tenantId = "tenant-enterprise-fintech";
    try {
      if (rawBody) tenantId = JSON.parse(rawBody).tenant_id || tenantId;
    } catch {}

    return {
      status: 200,
      body: {
        tenant_id: tenantId,
        status: "shredded",
        shred_latency_nanos: 39500, // 39.5 µs
        deleted_db_entries: 65,
        deleted_exact_cache: 92,
        deleted_vectors: 92,
        deleted_execution_tools: 18,
        deleted_learning_events: 12,
        verified_multi_store_erasure: true,
      },
    };
  }

  if (norm === "v1/rebuild") {
    return {
      status: 200,
      body: {
        status: "rebuilt",
        rehydrated_caches: 1420,
        rehydrated_vectors: 890,
        elapsed_ms: 42,
      },
    };
  }

  // 9. Invalidation Cascade
  if (norm === "v1/invalidation") {
    let sourceId = "src-openapi-v2";
    try {
      if (rawBody) sourceId = JSON.parse(rawBody).source_id || sourceId;
    } catch {}

    return {
      status: 200,
      body: {
        status: "invalidated",
        source_id: sourceId,
        evicted_cache_count: 1420,
      },
    };
  }

  // 10. Adaptive Learning Policies
  if (norm === "v1/learning/policies") {
    return {
      status: 200,
      body: {
        tenant_id: "tenant-enterprise-fintech",
        active_policy: {
          policy_id: "policy-v1-production-calibrated",
          threshold: 0.88,
          conformal_alpha: 0.001,
          evaluation_report: {
            is_statistically_sound: true,
            sample_size: 124,
            empirical_risk: 0.0,
          },
        },
        canary_policy: {
          percentage: 10,
          policy: {
            policy_id: "policy-v2-canary-experimental",
            threshold: 0.85,
            conformal_alpha: 0.001,
          },
        },
        total_feedback_events: 1284,
        audit_trail: [
          {
            timestamp: new Date(Date.now() - 3600000).toISOString(),
            action: "CANARY_DEPLOYED",
            percentage: 10,
          },
        ],
      },
    };
  }

  if (norm === "v1/learning/calibrate" || norm === "v1/learning/canary" || norm === "v1/learning/promote" || norm === "v1/learning/rollback" || norm === "v1/learning/feedback") {
    return {
      status: 200,
      body: {
        status: "ok",
        action: normalizedPath,
        timestamp: new Date().toISOString(),
        candidate_policy: {
          policy_id: "policy-v2-calibrated",
          threshold: 0.86,
          conformal_alpha: 0.001,
        },
      },
    };
  }

  // 11. Chaos Engineering
  if (norm === "v1/chaos/status" || norm === "v1/chaos/trip" || norm === "v1/chaos/reset") {
    return {
      status: 200,
      body: {
        status: "ok",
        circuit_breaker: norm.includes("trip") ? "OPEN" : "CLOSED",
        origin_failover: norm.includes("trip"),
        timestamp: new Date().toISOString(),
      },
    };
  }

  // 12. Cache entry deletion
  if (norm.startsWith("v1/caches/")) {
    return {
      status: 200,
      body: {
        deleted: true,
        key: normalizedPath.replace(/^v1\/caches\//, ""),
      },
    };
  }

  return null;
}
