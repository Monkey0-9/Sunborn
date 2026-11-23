"""
Sunborn Unified Developer and Operations CLI.

Provides command-line diagnostics, benchmarking, FinOps reporting,
and instant cryptographic tenant shredding.
"""

from __future__ import annotations

import argparse
import math
import os
import sys
import time
import urllib.error
import urllib.request

from .accel import is_c_accelerator_available
from .client import SunbornClient, _fast_dumps, _fast_loads


def _wilson_score_interval(successes: int, total: int, z: float = 1.96) -> tuple[float, float]:
    """Calculate 95% Wilson score confidence interval for a proportion."""
    if total == 0:
        return 0.0, 0.0
    p = successes / total
    denom = 1.0 + (z**2) / total
    center = (p + (z**2) / (2.0 * total)) / denom
    margin = (z * math.sqrt((p * (1.0 - p) + (z**2) / (4.0 * total)) / total)) / denom
    return max(0.0, center - margin), min(1.0, center + margin)


def cmd_doctor(args: argparse.Namespace) -> int:
    """Run health, connectivity, SIMD acceleration, and configuration checks."""
    print("=" * 60)
    print("  Sunborn Diagnostic Health Check (sunborn doctor)")
    print("=" * 60)

    url = args.url or os.environ.get("SUNBORN_BASE_URL", "http://localhost:8080/v1")
    clean_base = url.rstrip("/").removesuffix("/v1")
    api_key = args.key or os.environ.get("SUNBORN_API_KEY", "sunborn-local-dev-key")
    tenant_id = args.tenant or os.environ.get("SUNBORN_TENANT_ID", "default-tenant")

    print(f"[*] Gateway Base URL:   {url}")
    print(f"[*] Active Tenant ID:   {tenant_id}")
    print(f"[*] API Key Configured: {'[YES]' if api_key else '[NO]'}")

    # 1. C-SIMD Accelerator status
    c_accel = is_c_accelerator_available()
    print(
        f"[*] SIMD Accelerator:  {'[ENABLED] C-accelerated bitsets active' if c_accel else '[FALLBACK] Pure Python vectorized kernel active'}"
    )

    # 2. Probe /health
    health_url = f"{clean_base}/health"
    print(f"\n[*] Probing Gateway Health ({health_url})...", end=" ", flush=True)
    health_ok = False
    try:
        req = urllib.request.Request(health_url, headers={"User-Agent": "Sunborn-CLI/1.0.0"})
        t0 = time.time()
        with urllib.request.urlopen(req, timeout=args.timeout) as resp:
            latency_ms = (time.time() - t0) * 1000.0
            if resp.status == 200:
                print(f"[OK] ({latency_ms:.2f} ms)")
                health_ok = True
            else:
                print(f"[WARN] HTTP {resp.status}")
    except Exception as err:
        print(f"[FAIL] ({err})")

    # 3. Probe /ready or /v1/cache/graph/stats
    ready_url = f"{url.rstrip('/')}/cache/graph/stats"
    print(f"[*] Probing Engine Cache Stats ({ready_url})...", end=" ", flush=True)
    ready_ok = False
    try:
        req = urllib.request.Request(
            ready_url,
            headers={
                "User-Agent": "Sunborn-CLI/1.0.0",
                "x-api-key": api_key,
                "x-tenant-id": tenant_id,
            },
        )
        t0 = time.time()
        with urllib.request.urlopen(req, timeout=args.timeout) as resp:
            latency_ms = (time.time() - t0) * 1000.0
            if resp.status == 200:
                body = _fast_loads(resp.read())
                nodes = body.get("active_nodes", body.get("total_nodes", "N/A"))
                print(f"[OK] ({latency_ms:.2f} ms, active nodes: {nodes})")
                ready_ok = True
            else:
                print(f"[WARN] HTTP {resp.status}")
    except Exception as err:
        print(f"[FAIL] ({err})")

    print("\n" + "-" * 60)
    if health_ok and ready_ok:
        print("  Status: HEALTHY - Sunborn Gateway is fully operational.")
        print("-" * 60)
        return 0
    elif health_ok:
        print("  Status: DEGRADED - Gateway is running but cache endpoints failed.")
        print("-" * 60)
        return 1
    else:
        print("  Status: OFFLINE - Gateway unreachable. Ensure the gateway process is running.")
        print("-" * 60)
        return 2


def cmd_bench(args: argparse.Namespace) -> int:
    """Execute latency benchmark with percentiles and Wilson 95% confidence intervals."""
    print("=" * 60)
    print("  Sunborn High-Throughput Gateway Benchmark (sunborn bench)")
    print("=" * 60)

    url = args.url or os.environ.get("SUNBORN_BASE_URL", "http://localhost:8080/v1")
    api_key = args.key or os.environ.get("SUNBORN_API_KEY", "sunborn-local-dev-key")
    tenant_id = args.tenant or os.environ.get("SUNBORN_TENANT_ID", "default-tenant")

    requests_count = args.requests
    model = args.model

    print(f"[*] Target:      {url}/chat/completions")
    print(f"[*] Total Req:   {requests_count}")
    print(f"[*] Model:       {model}")
    print(f"[*] Tenant:      {tenant_id}")
    print("\nRunning benchmark probes...")

    client = SunbornClient(base_url=url, api_key=api_key, tenant_id=tenant_id, timeout=args.timeout)

    latencies: list[float] = []
    hits = 0
    t_start = time.time()

    prompt_corpus = [
        "What are the benefits of semantic caching?",
        "Explain causal dependency graph invalidation.",
        "How does Ed25519 signing attest cache freshness?",
        "What is the time complexity of roaring bitmap operations?",
    ]

    for i in range(requests_count):
        # Pick prompt to induce both exact hits and new queries
        prompt = prompt_corpus[i % len(prompt_corpus)]
        t0 = time.time()
        try:
            resp = client.chat_completion(model=model, prompt=prompt)
            latency = (time.time() - t0) * 1000.0
            latencies.append(latency)
            if resp.get("sunborn_cached"):
                hits += 1
        except Exception:
            latencies.append(args.timeout * 1000.0)

    total_time = time.time() - t_start
    if not latencies:
        print("[FAIL] No benchmark measurements completed.")
        return 1

    latencies.sort()
    n = len(latencies)
    p50 = latencies[int(n * 0.50)]
    p90 = latencies[int(n * 0.90)]
    p99 = latencies[int(n * 0.99)]
    hit_rate = (hits / n) * 100.0
    w_low, w_high = _wilson_score_interval(hits, n)
    rps = n / total_time if total_time > 0 else 0.0

    print("\n" + "=" * 60)
    print("  Benchmark Results")
    print("=" * 60)
    print(f"  Throughput:           {rps:8.1f} req/sec")
    print(f"  Latency p50:          {p50:8.2f} ms")
    print(f"  Latency p90:          {p90:8.2f} ms")
    print(f"  Latency p99:          {p99:8.2f} ms")
    print(f"  Cache Hit Rate:       {hit_rate:8.1f}% ({hits}/{n})")
    print(f"  Wilson 95% CI:        [{w_low * 100:.1f}%, {w_high * 100:.1f}%]")
    print("=" * 60)
    return 0


def cmd_finops(args: argparse.Namespace) -> int:
    """Retrieve and display FinOps spend report and token avoidance."""
    print("=" * 60)
    print("  Sunborn FinOps Cost Governance Report (sunborn finops)")
    print("=" * 60)

    url = args.url or os.environ.get("SUNBORN_BASE_URL", "http://localhost:8080/v1")
    api_key = args.key or os.environ.get("SUNBORN_API_KEY", "sunborn-local-dev-key")
    tenant_id = args.tenant or os.environ.get("SUNBORN_TENANT_ID", "default-tenant")

    client = SunbornClient(base_url=url, api_key=api_key, tenant_id=tenant_id, timeout=args.timeout)
    try:
        report = client.get_finops_report()
    except Exception as err:
        print(f"[FAIL] Unable to retrieve FinOps report: {err}")
        return 1

    total_queries = report.get("total_queries", 0)
    cached_queries = report.get("cached_queries", 0)
    avoided_tokens = report.get("avoided_tokens", 0)
    avoided_cost = report.get("avoided_cost_usd", 0.0)
    origin_cost = report.get("origin_cost_usd", 0.0)

    print(f"  Tenant Scope:         {tenant_id}")
    print(f"  Total Queries:        {total_queries:,}")
    print(f"  Cached Queries:       {cached_queries:,}")
    print(f"  Avoided Tokens:       {avoided_tokens:,}")
    print(f"  Avoided Cost:         ${avoided_cost:,.4f} USD")
    print(f"  Origin Spend:         ${origin_cost:,.4f} USD")
    if total_queries > 0:
        ratio = (cached_queries / total_queries) * 100.0
        print(f"  Cost Avoidance Rate:  {ratio:.2f}%")
    print("=" * 60)
    return 0


def cmd_shred(args: argparse.Namespace) -> int:
    """Trigger O(1) instant cryptographic shredding for GDPR Article 17 compliance."""
    print("=" * 60)
    print("  Sunborn Instant Cryptographic Shredding (sunborn shred)")
    print("=" * 60)

    if not args.tenant_id:
        print("[FAIL] --tenant-id is required for cryptographic shredding.")
        return 1

    url = args.url or os.environ.get("SUNBORN_BASE_URL", "http://localhost:8080/v1")
    admin_key = args.admin_key or os.environ.get("SUNBORN_ADMIN_KEY", "admin-master-key")
    quorum_key = args.quorum_key or os.environ.get("SUNBORN_QUORUM_KEY", "quorum-peer-key")

    print(f"[*] Target Tenant:    {args.tenant_id}")
    print(f"[*] Shred Endpoint:   {url.rstrip('/')}/governance/shred")

    headers = {
        "Content-Type": "application/json",
        "x-api-key": admin_key,
        "x-quorum-admin-key": quorum_key,
        "User-Agent": "Sunborn-CLI/1.0.0",
    }
    payload = {"tenant_id": args.tenant_id}

    shred_url = f"{url.rstrip('/')}/governance/shred"
    t0 = time.time()
    try:
        body = _fast_dumps(payload)
        req = urllib.request.Request(shred_url, data=body, headers=headers, method="POST")
        with urllib.request.urlopen(req, timeout=args.timeout) as resp:
            data = _fast_loads(resp.read())
            elapsed_ms = (time.time() - t0) * 1000.0

            status = data.get("status", "shredded")
            latency_ns = data.get("shred_latency_nanos", 0)
            verified = data.get("verified_multi_store_erasure", False)

            print("\n" + "=" * 60)
            print(f"  Shred Status:         {status.upper()}")
            print(f"  TDEK Zeroization:     {latency_ns:,} ns ({latency_ns / 1_000_000:.3f} ms)")
            print(
                f"  Multi-Store Verified: {'[YES] 0 residuals verified across all 5 tiers' if verified else '[PARTIAL]'}"
            )
            print(f"  Total Wall Time:      {elapsed_ms:.2f} ms")
            print("=" * 60)
            return 0 if (status == "shredded" and verified) else 1
    except Exception as err:
        print(f"[FAIL] Shred request failed: {err}")
        return 1


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="sunborn",
        description="Sunborn Enterprise Semantic AI Gateway Developer and Ops CLI",
    )
    parser.add_argument("-v", "--version", action="version", version="sunborn 1.0.0")

    subparsers = parser.add_subparsers(dest="command", help="Available subcommands")

    # doctor
    p_doctor = subparsers.add_parser("doctor", help="Run connectivity, SIMD, and health checks")
    p_doctor.add_argument("--url", default=None, help="Sunborn Gateway URL")
    p_doctor.add_argument("--key", default=None, help="Sunborn API key")
    p_doctor.add_argument("--tenant", default=None, help="Tenant identifier")
    p_doctor.add_argument("--timeout", type=float, default=5.0, help="Probe timeout in seconds")

    # bench
    p_bench = subparsers.add_parser("bench", help="Run latency benchmark with Wilson 95% confidence intervals")
    p_bench.add_argument("--url", default=None, help="Sunborn Gateway URL")
    p_bench.add_argument("--key", default=None, help="Sunborn API key")
    p_bench.add_argument("--tenant", default=None, help="Tenant identifier")
    p_bench.add_argument("--requests", type=int, default=20, help="Number of benchmark queries")
    p_bench.add_argument("--model", default="gpt-4o", help="Model identifier")
    p_bench.add_argument("--timeout", type=float, default=10.0, help="Per-request timeout")

    # finops
    p_finops = subparsers.add_parser("finops", help="Retrieve live FinOps token avoidance and cost report")
    p_finops.add_argument("--url", default=None, help="Sunborn Gateway URL")
    p_finops.add_argument("--key", default=None, help="Sunborn API key")
    p_finops.add_argument("--tenant", default=None, help="Tenant identifier")
    p_finops.add_argument("--timeout", type=float, default=5.0, help="Request timeout")

    # shred
    p_shred = subparsers.add_parser("shred", help="Trigger O(1) instant cryptographic tenant shredding")
    p_shred.add_argument("--tenant-id", required=True, help="Tenant to cryptographically shred")
    p_shred.add_argument("--url", default=None, help="Sunborn Gateway URL")
    p_shred.add_argument("--admin-key", default=None, help="Admin master key")
    p_shred.add_argument("--quorum-key", default=None, help="Quorum peer admin key")
    p_shred.add_argument("--timeout", type=float, default=10.0, help="Request timeout")

    args = parser.parse_args(argv)
    if not args.command:
        parser.print_help()
        return 0

    if args.command == "doctor":
        return cmd_doctor(args)
    elif args.command == "bench":
        return cmd_bench(args)
    elif args.command == "finops":
        return cmd_finops(args)
    elif args.command == "shred":
        return cmd_shred(args)

    return 0


if __name__ == "__main__":
    sys.exit(main())
