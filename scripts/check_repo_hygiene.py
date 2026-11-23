#!/usr/bin/env python3
"""Fail if the tracked tree contains scratch artifacts, caches, or secrets.

This is a public-repository gate, not a linter. It checks only tracked files
(``git ls-files``), so ignored build output does not cause noise.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path, PurePosixPath

FORBIDDEN_DIRS = {
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "node_modules",
    "target",
    ".venv",
    ".venv_smoke",
    "dist",
    "build",
}

FORBIDDEN_SUFFIXES = {
    ".pyc",
    ".ipynb",
    ".log",
    ".tmp",
    ".bak",
    ".rs.bk",
    ".pem",
    ".key",
    ".tfstate",
}

FORBIDDEN_NAMES = {
    ".env",
    ".env.local",
    ".env.production",
    "kubeconfig",
    "id_rsa",
    "id_ed25519",
}

ALLOWED_ENV_FILES = {".env.example"}


def tracked_files() -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        capture_output=True,
        check=True,
    )
    return [entry for entry in result.stdout.decode("utf-8", errors="replace").split("\x00") if entry]


def check(max_bytes: int) -> int:
    files = tracked_files()
    violations: list[str] = []

    for path in files:
        pure = PurePosixPath(path)
        if any(part in FORBIDDEN_DIRS for part in pure.parts):
            violations.append(f"cache/build directory tracked: {path}")
            continue

        name = pure.name
        if name in FORBIDDEN_NAMES:
            violations.append(f"secret or local environment file tracked: {path}")
            continue
        if name.startswith(".env.") and name not in ALLOWED_ENV_FILES:
            violations.append(f"environment file tracked: {path}")
            continue

        if pure.suffix.lower() in FORBIDDEN_SUFFIXES:
            violations.append(f"scratch or key file tracked: {path}")
            continue

        try:
            size = Path(path).stat().st_size
        except OSError:
            size = 0
        if size > max_bytes:
            violations.append(f"large tracked file ({size} bytes): {path}")

    if violations:
        print("REPO HYGIENE FAILED", file=sys.stderr)
        for violation in violations:
            print(f"  - {violation}", file=sys.stderr)
        return 1

    print(f"OK: {len(files)} tracked files; no scratch artifacts, caches, or secret files.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--max-bytes",
        type=int,
        default=5 * 1024 * 1024,
        help="maximum allowed size for a tracked file (default: 5 MiB)",
    )
    args = parser.parse_args()
    return check(args.max_bytes)


if __name__ == "__main__":
    sys.exit(main())
