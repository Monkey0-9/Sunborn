#!/usr/bin/env python3
"""
Sunborn Platform: Public Launch Readiness & Scale Verification Engine.Runs seven in-tree documentation, contract and client-interface checks.

SCOPE LIMIT — these checks are necessary but NOT sufficient for a public
launch, and this script is not a release authority. It does not run load,
fault-injection, penetration, capacity or disaster-recovery tests, it does not
verify the OpenAPI contract at runtime, and it makes no claim about the number
of concurrent users the system can support. A passing run means "the in-tree
docs, contract and client samples are internally consistent", nothing more.

In-Tree Verification Gates:
  1. Documentation & Relative Link Integrity (zero missing source paths)
  2. Launch Documentation & Claim Verification (zero unqualified claims)
  3. OpenAPI 3.1 Contract Parity & Specification Drift
  4. Enterprise Client Demonstration Suite (all 6 pillars verified)
  5. Python SDK Enterprise Integration Quickstart (dry-run self-test)
  6. Deterministic CycloneDX v1.5 Software Bill of Materials (SBOM) Generation
  7. Grafana Telemetry Dashboard Schema Validity

Usage:
  python scripts/verify_public_launch.py [--strict] [--json]
"""

import argparse
import json
import os
import subprocess
import sys
import time

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class Style:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    GREEN = "\033[32m"
    RED = "\033[31m"
    YELLOW = "\033[33m"
    CYAN = "\033[36m"
    GRAY = "\033[90m"
    WHITE = "\033[37m"


def run_stage(title: str, cmd: list[str], cwd: str = ROOT_DIR) -> tuple[bool, str]:
    t0 = time.perf_counter()
    try:
        proc = subprocess.run(
            cmd,
            cwd=cwd,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="ignore",
            timeout=120,
        )
        elapsed = time.perf_counter() - t0
        output = (proc.stdout or "") + (proc.stderr or "")
        return (proc.returncode == 0, f"{output.strip()} ({elapsed:.2f}s)")
    except Exception as e:
        return (False, f"Execution failed: {e}")


def verify_grafana_dashboard() -> tuple[bool, str]:
    dashboard_path = os.path.join(ROOT_DIR, "deploy", "grafana", "sunborn_gateway_dashboard.json")
    if not os.path.exists(dashboard_path):
        return (False, "Missing dashboard file")
    try:
        with open(dashboard_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        assert data.get("title") == "Sunborn Enterprise Gateway — Production Telemetry"
        assert len(data.get("panels", [])) >= 5
        return (True, f"Dashboard JSON valid with {len(data['panels'])} telemetry panels")
    except Exception as e:
        return (False, f"JSON parsing failed: {e}")


def main():
    parser = argparse.ArgumentParser(description="Sunborn Public Launch Verification Engine")
    parser.add_argument("--strict", action="store_true", help="Fail build on any warning")
    parser.add_argument("--json", action="store_true", help="Output machine-readable JSON")
    args = parser.parse_args()

    if not args.json:
        print(f"\n{Style.BOLD}{Style.CYAN}================================================================================{Style.RESET}")
        print(f"{Style.BOLD}{Style.WHITE}     S U N B O R N   P U B L I C   L A U N C H   V E R I F I C A T I O N{Style.RESET}")
        print(f"{Style.GRAY}  In-Tree Documentation, Contract & Client Consistency Checks{Style.RESET}")
        print(f"{Style.BOLD}{Style.CYAN}================================================================================{Style.RESET}\n")

    py_bin = sys.executable

    stages = [
        ("Strict Documentation Source Paths", [py_bin, "scripts/verify_doc_paths.py", "--strict"]),
        ("Launch Documentation Link & Claims Gate", [py_bin, "scripts/check_launch_docs.py"]),
        ("OpenAPI 3.1 Specification Parity", [py_bin, "scripts/check_openapi_drift.py"]),
        ("6-Pillar Enterprise Client Showcase", [py_bin, "scripts/client_showcase.py", "--self-test", "--json"]),
        ("Enterprise Python SDK Client Tour", [py_bin, "sdk/python/examples/enterprise_client_tour.py", "--dry-run"]),
        ("Deterministic CycloneDX v1.5 SBOM", [py_bin, "scripts/generate_sbom.py"]),
        ("Grafana Telemetry Dashboard Schema", None),
    ]

    results = []
    start_time = time.time()

    for idx, (name, cmd) in enumerate(stages, 1):
        if cmd is None:
            passed, detail = verify_grafana_dashboard()
        else:
            passed, detail = run_stage(name, cmd)

        results.append({"stage": name, "passed": passed, "detail": detail})
        status_str = f"{Style.GREEN}PASS{Style.RESET}" if passed else f"{Style.RED}FAIL{Style.RESET}"

        if not args.json:
            print(f"  [{idx}/{len(stages)}] {name:<42} : [{status_str}]")

    total_duration = time.time() - start_time
    all_passed = all(r["passed"] for r in results)

    if args.json:
        print(json.dumps({
            "verdict": (
                "IN_TREE_CHECKS_PASSED" if all_passed else "IN_TREE_CHECKS_FAILED"
            ),
            "release_authority": False,
            "note": "Not a launch approval. See docs/STATUS.md for the authoritative release decision and open gates.",
            "duration_seconds": round(total_duration, 3),
            "stages": results,
        }, indent=2))
    else:
        print(f"\n{Style.BOLD}{Style.CYAN}--------------------------------------------------------------------------------{Style.RESET}")
        if all_passed:
            print(f"{Style.BOLD}{Style.GREEN}  IN-TREE CHECKS PASSED: {len(stages)}/{len(stages)} ({total_duration:.2f}s){Style.RESET}")
            print(f"{Style.GRAY}  Not a launch approval. Load, security, fault-injection and capacity gates are not run here.{Style.RESET}")
        else:
            print(f"{Style.BOLD}{Style.RED}  IN-TREE CHECKS FAILED ({total_duration:.2f}s){Style.RESET}")
        print(f"{Style.BOLD}{Style.CYAN}================================================================================{Style.RESET}\n")

    sys.exit(0 if all_passed else 1)


if __name__ == "__main__":
    main()
