#!/usr/bin/env python3
"""Measure Sunborn gateway latency and cache behaviour over real HTTP.

This tool reports only what it observes from the client. It does not model
upstream model latency, multi-node scaling, TLS termination, or database cost;
those require a deployment shaped like the target environment and are called
out in the output's ``limitations`` list.

Examples
--------
    python benchmarks/sunborn_bench.py --help
    python benchmarks/sunborn_bench.py --dry-run
    SUNBORN_API_KEY=... python benchmarks/sunborn_bench.py \
        --base-url http://127.0.0.1:8080 --iterations 2000 --concurrency 32

The gateway must already be running. A missing origin provider is fine: the
gateway fabricates a mock origin for local runs, and the JSON output records
that the origin was whatever the gateway reported.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import platform
import statistics
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone


SCHEMA_VERSION = "sunborn-bench-v1"
DEFAULT_PROMPT = "Summarize the warranty terms for a returned laptop."


def percentile(sorted_values: list[float], fraction: float) -> float:
    """Nearest-rank percentile; avoids interpolating values that were not seen."""
    if not sorted_values:
        return 0.0
    index = max(0, min(len(sorted_values) - 1, int(round(fraction * (len(sorted_values) - 1)))))
    return sorted_values[index]


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Benchmark a running Sunborn gateway over HTTP.",
        epilog="Only client-observed measurements are reported. See the limitations in the output.",
    )
    parser.add_argument("--base-url", default=os.environ.get("SUNBORN_BASE_URL", "http://127.0.0.1:8080"))
    parser.add_argument("--api-key", default=os.environ.get("SUNBORN_API_KEY", ""))
    parser.add_argument("--tenant", default="tenant-alpha")
    parser.add_argument("--app", default="app-default")
    parser.add_argument("--model", default="gpt-4o")
    parser.add_argument(
        "--baseline",
        choices=("exact", "unique"),
        default="exact",
        help="exact: one warmed prompt; unique: a distinct prompt per request (forces misses)",
    )
    parser.add_argument("--iterations", type=int, default=1000, help="measured requests (default: 1000)")
    parser.add_argument("--concurrency", type=int, default=16, help="worker threads (default: 16)")
    parser.add_argument("--timeout", type=float, default=30.0, help="per-request timeout in seconds")
    parser.add_argument("--output-json", default="", help="write the full report to this path")
    parser.add_argument("--format", choices=("table", "json"), default="table")
    parser.add_argument("--dry-run", action="store_true", help="print the plan without sending requests")
    args = parser.parse_args(argv)

    for name in ("iterations", "concurrency"):
        if getattr(args, name) < 1:
            parser.error(f"--{name} must be at least 1")
    if args.timeout <= 0:
        parser.error("--timeout must be positive")
    return args


def plan(args: argparse.Namespace) -> dict:
    return {
        "schema_version": SCHEMA_VERSION,
        "dry_run": True,
        "base_url": args.base_url,
        "baseline": args.baseline,
        "iterations": args.iterations,
        "concurrency": args.concurrency,
        "tenant": args.tenant,
        "app": args.app,
        "model": args.model,
        "measured_requests": 0,
        "note": "No requests were sent. Remove --dry-run to measure a running gateway.",
    }


def post_request(args: argparse.Namespace, prompt: str) -> dict:
    payload = json.dumps(
        {
            "tenant_id": args.tenant,
            "application_id": args.app,
            "model": args.model,
            "prompt": prompt,
        }
    ).encode("utf-8")
    request = urllib.request.Request(
        f"{args.base_url.rstrip('/')}/v1/requests",
        data=payload,
        method="POST",
        headers={
            "content-type": "application/json",
            "x-api-key": args.api_key,
        },
    )
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=args.timeout) as response:
            body = response.read()
            status = response.status
    except urllib.error.HTTPError as error:
        body = error.read()
        status = error.code
    elapsed_ms = (time.perf_counter() - started) * 1000.0

    parsed = None
    try:
        parsed = json.loads(body) if body else None
    except json.JSONDecodeError:
        parsed = None
    return {"status": status, "elapsed_ms": elapsed_ms, "body": parsed}


def run(args: argparse.Namespace) -> dict:
    if not args.api_key:
        raise SystemExit(
            "No API key. Set SUNBORN_API_KEY or pass --api-key. "
            "Local dev default: test-key-alpha (tenant-alpha / app-default)."
        )

    warmup = None
    if args.baseline == "exact":
        # Warm one prompt so the measured window reflects the cached path.
        warmup = post_request(args, DEFAULT_PROMPT)
        if warmup["status"] != 200:
            raise SystemExit(f"Warm-up request failed with HTTP {warmup['status']}: {warmup['body']}")

    def one(index: int) -> dict:
        prompt = DEFAULT_PROMPT if args.baseline == "exact" else f"{DEFAULT_PROMPT} [case {index}]"
        return post_request(args, prompt)

    started = time.perf_counter()
    results: list[dict] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.concurrency) as pool:
        futures = [pool.submit(one, index) for index in range(args.iterations)]
        for future in concurrent.futures.as_completed(futures):
            results.append(future.result())
    wall_clock = time.perf_counter() - started

    ok = [item for item in results if item["status"] == 200]
    failures = [item for item in results if item["status"] != 200]
    latencies = sorted(item["elapsed_ms"] for item in ok)
    cached = sum(1 for item in ok if isinstance(item["body"], dict) and item["body"].get("cached"))
    server_durations = [
        float(item["body"]["duration_ms"])
        for item in ok
        if isinstance(item["body"], dict) and isinstance(item["body"].get("duration_ms"), (int, float))
    ]

    summary = {
        "schema_version": SCHEMA_VERSION,
        "dry_run": False,
        "timestamp_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "base_url": args.base_url,
        "baseline": args.baseline,
        "iterations": args.iterations,
        "concurrency": args.concurrency,
        "tenant": args.tenant,
        "app": args.app,
        "model": args.model,
        "environment": {
            "os": platform.platform(),
            "python": platform.python_version(),
        },
        "warmup": warmup,
        "results": {
            "successful_requests": len(ok),
            "failed_requests": len(failures),
            "cache_hits": cached,
            "cache_hit_ratio": round(cached / len(ok), 4) if ok else 0.0,
            "wall_clock_sec": round(wall_clock, 3),
            "throughput_rps": round(len(ok) / wall_clock, 1) if wall_clock > 0 else 0.0,
            "latency_ms": {
                "p50": round(percentile(latencies, 0.50), 2),
                "p90": round(percentile(latencies, 0.90), 2),
                "p95": round(percentile(latencies, 0.95), 2),
                "p99": round(percentile(latencies, 0.99), 2),
                "max": round(max(latencies), 2) if latencies else 0.0,
                "mean": round(statistics.fmean(latencies), 2) if latencies else 0.0,
            },
            "server_duration_ms": {
                "p50": round(percentile(sorted(server_durations), 0.50), 2),
                "p99": round(percentile(sorted(server_durations), 0.99), 2),
            },
        },
        "failures": [
            {"status": item["status"], "body": item["body"]} for item in failures[:10]
        ],
        "limitations": [
            "Client-observed latency includes the client's own CPU and the loopback/network path.",
            "One gateway process was contacted. No multi-node scaling or failover was exercised.",
            "Upstream model latency is excluded: a local run normally uses the gateway's mock origin.",
            "TLS, persistent database, and distributed cache cost depend on the deployment and were not modelled.",
            "Do not extrapolate these numbers to a user count; multiply by measured horizontal scaling instead.",
        ],
    }

    canonical = json.dumps(summary, sort_keys=True, separators=(",", ":")).encode("utf-8")
    summary["run_digest_sha256"] = hashlib.sha256(canonical).hexdigest()
    return summary


def print_table(report: dict) -> None:
    results = report["results"]
    latency = results["latency_ms"]
    print(f"Sunborn benchmark  ({report['timestamp_utc']})")
    print(f"  baseline      : {report['baseline']}")
    print(f"  requests      : {results['successful_requests']} ok / {results['failed_requests']} failed")
    print(f"  concurrency   : {report['concurrency']}")
    print(f"  throughput    : {results['throughput_rps']} req/s")
    print(f"  latency p50   : {latency['p50']} ms")
    print(f"  latency p95   : {latency['p95']} ms")
    print(f"  latency p99   : {latency['p99']} ms")
    print(f"  cache hits    : {results['cache_hits']} ({results['cache_hit_ratio'] * 100:.1f}%)")
    print(f"  digest        : sha256:{report['run_digest_sha256']}")
    print("  limitations   : " + report["limitations"][0])
    if results["failed_requests"]:
        print("  first failure : " + json.dumps(report["failures"][0])[:300])


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    if args.dry_run:
        report = plan(args)
        print(json.dumps(report, indent=2))
        return 0

    report = run(args)
    if args.output_json:
        with open(args.output_json, "w", encoding="utf-8") as handle:
            json.dump(report, handle, indent=2)
            handle.write("\n")
    if args.format == "json":
        print(json.dumps(report, indent=2))
    else:
        print_table(report)
    return 0 if report["results"]["failed_requests"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
