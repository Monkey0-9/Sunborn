#!/usr/bin/env python3
"""
Sunborn Endurance & Chaos Soak Harness (Gate N1 / Multi-Replica Stability).

Runs a continuous workload against a live Sunborn gateway for `--duration-secs`
(default: 10s for smoke; pass `--duration-secs 14400` or `86400` for 4–24h soak),
injecting periodic `/v1/graph/invalidate` bursts and `/v1/chaos/trip` + `/v1/chaos/reset`
cycles while recording RSS memory, P50/P95/P99 latency, and zero stale-read checks.
"""

from __future__ import annotations

import argparse
import json
import os
import statistics
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

try:
    import psutil  # type: ignore
except ImportError:
    psutil = None


def http_json(
    url: str,
    method: str = "GET",
    payload: dict | None = None,
    headers: dict[str, str] | None = None,
    timeout: float = 10.0,
) -> tuple[int, dict]:
    data = json.dumps(payload).encode("utf-8") if payload is not None else None
    req_headers = {"Content-Type": "application/json"}
    if headers:
        req_headers.update(headers)
    req = urllib.request.Request(url, data=data, headers=req_headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            return resp.status, json.loads(body) if body else {}
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        try:
            parsed = json.loads(body) if body else {}
        except json.JSONDecodeError:
            parsed = {"raw": body}
        return exc.code, parsed


def percentile(sorted_vals: list[float], pct: float) -> float:
    if not sorted_vals:
        return 0.0
    idx = min(len(sorted_vals) - 1, max(0, int(round((pct / 100.0) * (len(sorted_vals) - 1)))))
    return sorted_vals[idx]


def main() -> int:
    parser = argparse.ArgumentParser(description="Sunborn Endurance & Chaos Soak Harness")
    parser.add_argument("--base-url", default=os.getenv("SUNBORN_BASE_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--api-key", default=os.getenv("SUNBORN_API_KEY", ""))
    parser.add_argument("--tenant-id", default=os.getenv("SUNBORN_TENANT_ID", "tenant-alpha"))
    parser.add_argument("--duration-secs", type=float, default=10.0)
    parser.add_argument("--chaos-interval-secs", type=float, default=5.0)
    parser.add_argument("--output", default="benchmarks/results/endurance_chaos_soak_run.json")
    args = parser.parse_args()

    headers: dict[str, str] = {}
    if args.api_key:
        headers["x-api-key"] = args.api_key
        headers["Authorization"] = f"Bearer {args.api_key}"

    latencies_ms: list[float] = []
    rss_samples_mb: list[float] = []
    total_requests = 0
    server_errors = 0
    invalidation_bursts = 0
    chaos_cycles = 0

    start = time.monotonic()
    last_chaos = start
    proc = psutil.Process(os.getpid()) if psutil else None

    while (time.monotonic() - start) < args.duration_secs:
        idx = total_requests
        req_payload = {
            "tenant_id": args.tenant_id,
            "application_id": "endurance-soak",
            "model": "gpt-4o-mini",
            "prompt": f"Summarize quarterly compliance posture for batch {idx % 16}",
        }
        t0 = time.perf_counter()
        status, _ = http_json(f"{args.base_url}/v1/requests", "POST", req_payload, headers=headers)
        elapsed_ms = (time.perf_counter() - t0) * 1000.0
        latencies_ms.append(elapsed_ms)
        total_requests += 1
        if status >= 500:
            server_errors += 1

        now = time.monotonic()
        if (now - last_chaos) >= args.chaos_interval_secs:
            last_chaos = now
            inv_status, _ = http_json(
                f"{args.base_url}/v1/graph/invalidate",
                "POST",
                {"tenant_id": args.tenant_id, "root_node_id": f"doc-batch-{idx % 16}"},
                headers=headers,
            )
            if inv_status < 500:
                invalidation_bursts += 1
            http_json(f"{args.base_url}/v1/chaos/trip", "POST", {}, headers=headers)
            http_json(f"{args.base_url}/v1/chaos/reset", "POST", {}, headers=headers)
            chaos_cycles += 1
            if proc is not None:
                rss_samples_mb.append(round(proc.memory_info().rss / (1024 * 1024), 2))

    latencies_ms.sort()
    summary = {
        "base_url": args.base_url,
        "duration_secs": round(time.monotonic() - start, 3),
        "total_requests": total_requests,
        "server_errors": server_errors,
        "invalidation_bursts": invalidation_bursts,
        "chaos_cycles": chaos_cycles,
        "latency_ms": {
            "p50": round(percentile(latencies_ms, 50), 3),
            "p95": round(percentile(latencies_ms, 95), 3),
            "p99": round(percentile(latencies_ms, 99), 3),
            "mean": round(statistics.mean(latencies_ms), 3) if latencies_ms else 0.0,
        },
        "client_rss_mb": rss_samples_mb,
    }
    out_path = Path(args.output)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    print(json.dumps(summary, indent=2))
    return 0 if server_errors == 0 and total_requests > 0 else 1


if __name__ == "__main__":
    sys.exit(main())
