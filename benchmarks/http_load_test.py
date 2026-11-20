#!/usr/bin/env python3
"""Real-HTTP load test for a running Sunborn Gateway.

Why this exists
---------------
`benchmarks/production_soak_results.json` is produced by
`crates/sunborn-gateway/tests/concurrent_multiworker_soak_test.rs`, which drives
the router through `tower::ServiceExt::oneshot` in-process: no socket, no HTTP
parser, no TLS, no network stack. Those numbers are useful for tracking
regressions in handler code, but they are **not** evidence about a deployed
gateway, and they must not be quoted as throughput for a public launch.

This harness measures the deployed thing: it opens real TCP connections,
speaks real HTTP/1.1 with keep-alive, and records client-observed latency.

Usage
-----
    python benchmarks/http_load_test.py --requests 20000 --concurrency 32
    python benchmarks/http_load_test.py --url http://127.0.0.1:8080 --json out.json

Reported metrics come from one measurement, not a model:
    * throughput = completed_requests / wall_clock_seconds
    * percentiles = nearest-rank over recorded per-request latencies
    * errors are counted separately and the run exits non-zero if any occurred
"""

from __future__ import annotations

import argparse
import http.client
import json
import statistics
import sys
import threading
import time
import urllib.parse
import uuid
from typing import List, Optional, Tuple

DEFAULT_PROMPT = "Explain in one sentence why cache invalidation is hard."


def percentile(sorted_values: List[float], pct: float) -> float:
    """Nearest-rank percentile on an already-sorted list."""
    if not sorted_values:
        return 0.0
    k = max(0, min(len(sorted_values) - 1, int(round((pct / 100.0) * len(sorted_values) + 0.5)) - 1))
    return sorted_values[k]


class Worker(threading.Thread):
    """One keep-alive connection issuing a contiguous slice of requests."""

    def __init__(
        self,
        host: str,
        port: int,
        path: str,
        payload: bytes,
        headers: dict,
        count: int,
        latencies: List[float],
        errors: List[str],
        cached_hits: List[int],
        barrier: threading.Barrier,
        base_payload: Optional[dict] = None,
    ) -> None:
        super().__init__(daemon=True)
        self.host, self.port, self.path = host, port, path
        self.payload, self.headers = payload, headers
        # When set, every request gets a distinct prompt, so the cache key
        # differs each time and the window exercises the miss path.
        self.base_payload = base_payload
        self.count = count
        self.latencies, self.errors, self.cached_hits = latencies, errors, cached_hits
        self.barrier = barrier

    def run(self) -> None:
        conn = http.client.HTTPConnection(self.host, self.port, timeout=10.0)
        local: List[float] = []
        hits = 0
        errs: List[str] = []
        # All threads start together so the reported window is genuinely
        # concurrent instead of a staggered ramp.
        self.barrier.wait()
        try:
            for _ in range(self.count):
                if self.base_payload is not None:
                    body = json.dumps(
                        {**self.base_payload, "prompt": f"{self.base_payload['prompt']} {uuid.uuid4().hex}"}
                    ).encode("utf-8")
                else:
                    body = self.payload
                t0 = time.perf_counter()
                try:
                    conn.request("POST", self.path, body=body, headers=self.headers)
                    resp = conn.getresponse()
                    body = resp.read()
                    elapsed_ms = (time.perf_counter() - t0) * 1000.0
                    if resp.status != 200:
                        errs.append(f"HTTP {resp.status}: {body[:80]!r}")
                        continue
                    local.append(elapsed_ms)
                    if b'"cached":true' in body:
                        hits += 1
                except Exception as exc:  # connection reset, timeout, ...
                    errs.append(f"{type(exc).__name__}: {exc}")
                    try:
                        conn.close()
                    except Exception:
                        pass
                    conn = http.client.HTTPConnection(self.host, self.port, timeout=10.0)
        finally:
            conn.close()
            self.latencies.extend(local)
            self.errors.extend(errs)
            self.cached_hits.append(hits)


def build_request(
    tenant: str, app_id: str, model: str, prompt: str, unique_prompts: bool = False
) -> Tuple[str, bytes, dict]:
    path = "/v1/requests"
    payload = json.dumps(
        {
            "tenant_id": tenant,
            "application_id": app_id,
            "model": model,
            "prompt": prompt,
            "request_id": str(uuid.uuid4()),
        }
    ).encode("utf-8")
    headers = {"Content-Type": "application/json", "Connection": "keep-alive"}
    return path, payload, headers


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--url", default="http://127.0.0.1:8080")
    ap.add_argument("--api-key", default="sunborn-admin-key")
    ap.add_argument("--tenant", default="system-admin")
    ap.add_argument("--app-id", default="admin-ops")
    ap.add_argument("--model", default="gpt-4o")
    ap.add_argument("--prompt", default=DEFAULT_PROMPT)
    ap.add_argument("--requests", type=int, default=10000)
    ap.add_argument("--concurrency", type=int, default=32)
    ap.add_argument("--json", dest="json_out", default=None)
    ap.add_argument("--label", default="local")
    ap.add_argument(
        "--unique-prompts",
        action="store_true",
        help="give every request a distinct prompt so the run measures the cache-miss path",
    )
    args = ap.parse_args()

    parsed = urllib.parse.urlparse(args.url)
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or 80

    path, payload, headers = build_request(args.tenant, args.app_id, args.model, args.prompt)
    headers["x-api-key"] = args.api_key

    # One warmup request so the measured window is the steady state (cache
    # populated, connection pool warm), which is what a user population sees.
    warm = http.client.HTTPConnection(host, port, timeout=10.0)
    warm.request("POST", path, body=payload, headers=headers)
    warm_resp = warm.getresponse()
    warm_body = warm_resp.read()
    warm.close()
    if warm_resp.status != 200:
        print(f"ERROR: warmup request failed with HTTP {warm_resp.status}: {warm_body[:200]!r}", file=sys.stderr)
        return 2

    per_worker = args.requests // args.concurrency
    if per_worker == 0:
        print("ERROR: --requests must be at least --concurrency", file=sys.stderr)
        return 2
    total = per_worker * args.concurrency

    latencies: List[float] = []
    errors: List[str] = []
    cached_hits: List[int] = []
    barrier = threading.Barrier(args.concurrency + 1)
    base_payload = None
    if args.unique_prompts:
        base_payload = {
            "tenant_id": args.tenant,
            "application_id": args.app_id,
            "model": args.model,
            "prompt": args.prompt,
        }
    workers = [
        Worker(
            host,
            port,
            path,
            payload,
            headers,
            per_worker,
            latencies,
            errors,
            cached_hits,
            barrier,
            base_payload,
        )
        for _ in range(args.concurrency)
    ]

    started = time.perf_counter()
    for w in workers:
        w.start()
    barrier.wait()  # release every worker at once
    for w in workers:
        w.join()
    elapsed = time.perf_counter() - started

    ok = len(latencies)
    if ok == 0:
        print(f"ERROR: no successful requests ({len(errors)} errors); first: {errors[0] if errors else 'n/a'}", file=sys.stderr)
        return 1

    ordered = sorted(latencies)
    hits = sum(cached_hits)
    result = {
        "benchmark_name": "Sunborn Gateway real-HTTP load test",
        "label": args.label,
        "transport": "HTTP/1.1 over TCP, keep-alive, client-observed latency",
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "environment": {
            "os": sys.platform,
            "python": sys.version.split()[0],
            "logical_cores": __import__("os").cpu_count(),
        },
        "workload": {
            "endpoint": path,
            "total_requests_attempted": total,
            "concurrency_workers": args.concurrency,
            "tenant": args.tenant,
            "model": args.model,
            "cache_mode": "miss (unique prompt per request)" if args.unique_prompts else "hit (single warm prompt)",
        },
        "results": {
            "successful_requests": ok,
            "failed_requests": len(errors),
            "cache_hits": hits,
            "cache_hit_ratio": round(hits / ok, 4),
            "wall_clock_sec": round(elapsed, 3),
            "throughput_rps": round(ok / elapsed, 1),
            "latency_p50_ms": round(percentile(ordered, 50), 3),
            "latency_p90_ms": round(percentile(ordered, 90), 3),
            "latency_p95_ms": round(percentile(ordered, 95), 3),
            "latency_p99_ms": round(percentile(ordered, 99), 3),
            "latency_max_ms": round(ordered[-1], 3),
            "latency_mean_ms": round(statistics.fmean(ordered), 3),
        },
    }

    print(json.dumps(result, indent=2))
    if args.json_out:
        with open(args.json_out, "w", encoding="utf-8") as fh:
            json.dump(result, fh, indent=2)
            fh.write("\n")
        print(f"written: {args.json_out}", file=sys.stderr)

    if errors:
        print(f"FAIL: {len(errors)} request(s) failed; first: {errors[0]}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
