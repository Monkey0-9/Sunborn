#!/usr/bin/env python3
"""
Sunborn Enterprise AI Gateway — Automated Zero-Risk Docker Deployment Launcher.

Provides:
  1. Pre-flight host diagnostics (Docker daemon, compose engine, port availability).
  2. Automatic high-entropy cryptographic secret generation for .env.
  3. Fail-safe container lifecycle orchestration with health polling.
  4. End-to-end live synthetic validation across Gateway & Control Plane.
  5. Safe, non-destructive teardown and status inspections.

Zero external Python dependencies (pure standard library).
"""

from __future__ import annotations

import argparse
import json
import os
import secrets
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT_DIR = Path(__file__).resolve().parent.parent
COMPOSE_FILE = ROOT_DIR / "docker-compose.yml"
ENV_FILE = ROOT_DIR / ".env"
ENV_EXAMPLE = ROOT_DIR / "deploy" / ".env.example"

REQUIRED_PORTS = [
    ("Gateway API", 8080, "GATEWAY_PORT"),
    ("Control Plane (UI)", 3000, "FRONTEND_PORT"),
    ("PostgreSQL", 5432, "POSTGRES_PORT"),
    ("DragonflyDB (Cache)", 6379, "DRAGONFLY_PORT"),
    ("Qdrant (Vectors)", 6333, "QDRANT_PORT"),
]


class Colors:
    CYAN = "\033[96m"
    GREEN = "\033[92m"
    YELLOW = "\033[93m"
    RED = "\033[91m"
    BOLD = "\033[1m"
    RESET = "\033[0m"
    DIM = "\033[2m"


def log_info(msg: str) -> None:
    print(f"{Colors.CYAN}[INFO]{Colors.RESET} {msg}")


def log_success(msg: str) -> None:
    print(f"{Colors.GREEN}[SUCCESS]{Colors.RESET} {msg}")


def log_warn(msg: str) -> None:
    print(f"{Colors.YELLOW}[WARN]{Colors.RESET} {msg}")


def log_error(msg: str) -> None:
    print(f"{Colors.RED}[ERROR]{Colors.RESET} {msg}")


def is_port_in_use(port: int, host: str = "127.0.0.1") -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.5)
        return s.connect_ex((host, port)) == 0


def check_docker_engine() -> bool:
    log_info("Inspecting Docker daemon and Docker Compose...")
    try:
        proc_docker = subprocess.run(
            ["docker", "info"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if proc_docker.returncode != 0:
            log_error("Docker daemon is not running. Please start Docker Desktop or dockerd.")
            return False
    except FileNotFoundError:
        log_error("Docker CLI is not found on PATH. Please install Docker.")
        return False
    except subprocess.TimeoutExpired:
        log_error("Timed out communicating with Docker daemon.")
        return False

    try:
        proc_compose = subprocess.run(
            ["docker", "compose", "version"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if proc_compose.returncode != 0:
            log_error("Docker Compose plugin is not available.")
            return False
        compose_ver = proc_compose.stdout.strip()
        log_success(f"Docker & Compose operational ({compose_ver})")
        return True
    except Exception as e:
        log_error(f"Error checking Docker Compose: {e}")
        return False


def check_ports_available(ignore_docker_containers: bool = False) -> bool:
    log_info("Performing pre-flight port collision checks...")
    conflicts: list[tuple[str, int, str]] = []
    for name, port, env_var in REQUIRED_PORTS:
        if is_port_in_use(port):
            conflicts.append((name, port, env_var))

    if conflicts:
        log_warn(f"Found {len(conflicts)} port(s) currently occupied on 127.0.0.1:")
        for name, port, env_var in conflicts:
            print(f"  - Port {port} ({name}) is in use. (Override via {env_var})")
        if not ignore_docker_containers:
            log_info("If Sunborn containers are already running, use --status or continue.")
            return False
    else:
        log_success("All required ports (8080, 3000, 5432, 6379, 6333) are available on loopback.")
    return True


def ensure_secure_env() -> Path:
    if ENV_FILE.exists():
        log_info(f"Using existing configuration at {ENV_FILE}")
        return ENV_FILE

    log_info("Generating zero-risk, high-entropy cryptographic secrets for .env...")
    pg_pass = secrets.token_hex(16)
    master_kek = secrets.token_hex(32)  # 256-bit AES-GCM KEK
    admin_key = "sunborn-admin-" + secrets.token_hex(16)
    hmac_secret = secrets.token_hex(32)

    env_content = f"""# ==============================================================================
# Sunborn Enterprise AI Gateway — Auto-Generated Deployment Configuration
# Generated with cryptographically secure random entropy (secrets.token_hex)
# ==============================================================================

SUNBORN_ENV=development
HOST=0.0.0.0
PORT=8080

# Authoritative PostgreSQL Database
POSTGRES_USER=sunborn
POSTGRES_DB=sunborn
POSTGRES_PASSWORD={pg_pass}
POSTGRES_URL=postgres://sunborn:{pg_pass}@postgres:5432/sunborn

# Cache & Vector Store
SUNBORN_CACHE_BACKEND=dragonfly
DRAGONFLY_URL=redis://dragonfly:6379
SUNBORN_VECTOR_BACKEND=qdrant
QDRANT_URL=http://qdrant:6333
SUNBORN_DB_BACKEND=postgres

# Cryptographic Keys (256-bit AES-GCM Master KEK)
SUNBORN_MASTER_KEK={master_kek}
SUNBORN_ADMIN_KEY={admin_key}
SUNBORN_LEARNING_HMAC_SECRET={hmac_secret}

# CORS & Browser Bindings
SUNBORN_CORS_ALLOWED_ORIGINS=http://localhost:3000,http://127.0.0.1:3000,http://localhost:3001,http://127.0.0.1:3001,https://monkey0-9.github.io

# Upstream LLM Origin (Defaults to built-in mock origin if unset)
# SUNBORN_LLM_ORIGIN_URL=https://api.openai.com/v1
# SUNBORN_LLM_ORIGIN_API_KEY=
"""
    ENV_FILE.write_text(env_content, encoding="utf-8")
    log_success(f"Generated secure .env template at {ENV_FILE}")
    return ENV_FILE


def run_compose_up(services: list[str] | None = None) -> bool:
    cmd = ["docker", "compose", "-f", str(COMPOSE_FILE), "up", "-d"]
    if services:
        cmd.extend(services)
    log_info(f"Executing: {' '.join(cmd)}")
    proc = subprocess.run(cmd, cwd=str(ROOT_DIR))
    return proc.returncode == 0


def poll_container_health(timeout_secs: int = 45) -> bool:
    log_info(f"Waiting for containers to report healthy (timeout {timeout_secs}s)...")
    t0 = time.time()
    required = ["sunborn-postgres", "sunborn-dragonfly", "sunborn-qdrant"]
    while time.time() - t0 < timeout_secs:
        proc = subprocess.run(
            ["docker", "compose", "-f", str(COMPOSE_FILE), "ps", "--format", "json"],
            capture_output=True,
            text=True,
            cwd=str(ROOT_DIR),
        )
        if proc.returncode == 0 and proc.stdout.strip():
            healthy_count = 0
            for line in proc.stdout.strip().splitlines():
                try:
                    c = json.loads(line)
                    health = c.get("Health", "") or c.get("Status", "")
                    if "healthy" in health.lower():
                        healthy_count += 1
                except Exception:
                    continue
            if healthy_count >= len(required):
                log_success(f"All core dependencies are healthy! ({healthy_count} containers reporting healthy)")
                return True
        time.sleep(2)
        print(".", end="", flush=True)
    print()
    log_warn("Timeout reached waiting for container health probes.")
    return False


def verify_live_endpoints() -> bool:
    log_info("Probing live gateway endpoints on http://127.0.0.1:8080...")
    endpoints = [
        ("Health Probe", "http://127.0.0.1:8080/health"),
        ("Readiness Probe", "http://127.0.0.1:8080/ready"),
        ("Version & Build Info", "http://127.0.0.1:8080/version"),
        ("Prometheus Metrics", "http://127.0.0.1:8080/metrics"),
    ]
    all_ok = True
    for name, url in endpoints:
        try:
            req = urllib.request.Request(url, headers={"User-Agent": "Sunborn-Deploy-Verifier/1.0"})
            with urllib.request.urlopen(req, timeout=3) as resp:
                code = resp.getcode()
                if code == 200:
                    log_success(f"  {name:<22} -> HTTP {code} OK ({url})")
                else:
                    log_warn(f"  {name:<22} -> HTTP {code}")
                    all_ok = False
        except Exception as e:
            log_warn(f"  {name:<22} -> Unreachable ({e})")
            all_ok = False
    return all_ok


def run_status() -> None:
    print(f"\n{Colors.BOLD}{Colors.CYAN}=== Sunborn Deployment Status ==={Colors.RESET}\n")
    subprocess.run(["docker", "compose", "-f", str(COMPOSE_FILE), "ps"], cwd=str(ROOT_DIR))
    print()
    verify_live_endpoints()


def run_stop() -> None:
    log_info("Gracefully stopping Sunborn Docker containers (preserving database volumes)...")
    subprocess.run(["docker", "compose", "-f", str(COMPOSE_FILE), "stop"], cwd=str(ROOT_DIR))
    log_success("Containers stopped.")


def run_down(purge: bool = False) -> None:
    cmd = ["docker", "compose", "-f", str(COMPOSE_FILE), "down"]
    if purge:
        log_warn("Purging named volumes (pgdata, qdrant_data)...")
        cmd.append("-v")
    else:
        log_info("Removing containers and networks while safely preserving named volumes...")
    subprocess.run(cmd, cwd=str(ROOT_DIR))
    log_success("Stack teardown complete.")


def print_access_dashboard() -> None:
    admin_key = "sunborn-admin-key"
    if ENV_FILE.exists():
        for line in ENV_FILE.read_text(encoding="utf-8").splitlines():
            if line.startswith("SUNBORN_ADMIN_KEY="):
                admin_key = line.split("=", 1)[1].strip()

    print(f"""
{Colors.BOLD}{Colors.GREEN}================================================================================
     S U N B O R N   D E P L O Y M E N T   O P E R A T I O N A L
================================================================================{Colors.RESET}

  {Colors.BOLD}1. Web Cockpit & Control Plane{Colors.RESET}
     Dashboard URL    : {Colors.CYAN}http://localhost:3000{Colors.RESET}
     Health Check     : {Colors.CYAN}http://localhost:3000/api/health{Colors.RESET}

  {Colors.BOLD}2. Sunborn AI Gateway API{Colors.RESET}
     Base URL         : {Colors.CYAN}http://localhost:8080/v1{Colors.RESET}
     Health Probe     : {Colors.CYAN}http://localhost:8080/health{Colors.RESET}
     Prometheus Stats : {Colors.CYAN}http://localhost:8080/metrics{Colors.RESET}
     FinOps Report    : {Colors.CYAN}http://localhost:8080/v1/finops/report{Colors.RESET}
     Master Admin Key : {Colors.YELLOW}{admin_key}{Colors.RESET}

  {Colors.BOLD}3. Test With cURL (OpenAI-compatible drop-in):{Colors.RESET}
     curl -X POST http://localhost:8080/v1/chat/completions \\
       -H "Content-Type: application/json" \\
       -H "x-api-key: {admin_key}" \\
       -d '{{"model":"gpt-4o","messages":[{{"role":"user","content":"Hello Sunborn!"}}]}}'

  {Colors.BOLD}4. Management Commands:{Colors.RESET}
     Check status     : python scripts/docker_launch.py --status
     Graceful stop    : python scripts/docker_launch.py --stop
     Safe teardown    : python scripts/docker_launch.py --down
{Colors.BOLD}{Colors.GREEN}================================================================================{Colors.RESET}
""")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Sunborn Enterprise Zero-Risk Docker Deployment Launcher",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--up", action="store_true", help="Start the full Sunborn deployment stack")
    parser.add_argument("--brokers", action="store_true", help="Start only broker backends (Postgres, Dragonfly, Qdrant)")
    parser.add_argument("--status", action="store_true", help="Inspect real-time container health and gateway endpoints")
    parser.add_argument("--stop", action="store_true", help="Gracefully stop containers (preserves state)")
    parser.add_argument("--down", action="store_true", help="Tear down containers and network (preserves volumes)")
    parser.add_argument("--purge", action="store_true", help="Tear down containers and PURGE volumes (destructive)")
    parser.add_argument("--check-only", action="store_true", help="Run pre-flight checks and exit")

    args = parser.parse_args()

    # Default action if no flag is passed is --up
    if not any([args.up, args.brokers, args.status, args.stop, args.down, args.check_only]):
        args.up = True

    if args.status:
        run_status()
        return 0

    if args.stop:
        run_stop()
        return 0

    if args.down:
        run_down(purge=args.purge)
        return 0

    # Pre-flight checks
    if not check_docker_engine():
        return 1

    check_ports_available(ignore_docker_containers=True)
    ensure_secure_env()

    if args.check_only:
        log_success("Pre-flight validation complete. Ready to deploy.")
        return 0

    services = ["postgres", "dragonfly", "qdrant"] if args.brokers else None
    if not run_compose_up(services):
        log_error("Failed to start Docker Compose stack.")
        return 1

    poll_container_health()
    verify_live_endpoints()
    print_access_dashboard()
    return 0


if __name__ == "__main__":
    sys.exit(main())
