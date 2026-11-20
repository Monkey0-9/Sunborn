#!/usr/bin/env python3
"""
Sunborn Documentation Governance: Path Verifier.

Scans Markdown under docs/ and the root README for backticked repository
source paths and verifies they exist on disk.

By default this is advisory (warnings, exit 0). Pass --strict to fail the
build when a referenced source path is missing.
"""

import os
import re
import sys

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

PATH_PATTERN = re.compile(
    r"`((?:crates|apps|frontend|deploy|research|sdk|scripts|migrations|tests|benchmarks)/[a-zA-Z0-9_\-\./\[\]]+)`"
)


def collect_markdown_files():
    files = []
    readme = os.path.join(ROOT_DIR, "README.md")
    if os.path.exists(readme):
        files.append(readme)
    docs_dir = os.path.join(ROOT_DIR, "docs")
    for dirpath, _, filenames in os.walk(docs_dir):
        # Exclude ephemeral AI planning scratchspecs in docs/superpowers/
        if "superpowers" in dirpath.split(os.sep):
            continue
        for name in filenames:
            # 43_MISSING_IMPLEMENTATIONS.md explicitly catalogs non-existent legacy references
            if name.endswith(".md") and name != "43_MISSING_IMPLEMENTATIONS.md":
                files.append(os.path.join(dirpath, name))
    return files


# Historical junk/fake artifacts deleted during audit remediation and cited in audit logs
REMEDIATION_DELETED_ITEMS = {
    "frontend/src/lib/standalone-simulator.ts",
    "research/results/baseline_backup",
    "scripts/audit_licenses.py",
}

# Ephemeral build, package, and git-ignored targets not tracked in repository
EPHEMERAL_BUILD_TARGETS = {
    "frontend/.next",
    "frontend/node_modules",
    "frontend/tsconfig.tsbuildinfo",
    "sdk/python/build",
    "sdk/python/dist",
    "sdk/python/sunborn.egg-info",
}


def verify(strict: bool) -> int:
    checked = set()
    missing = set()

    for md in collect_markdown_files():
        with open(md, "r", encoding="utf-8", errors="ignore") as handle:
            content = handle.read()
        for path in PATH_PATTERN.findall(content):
            clean = path.rstrip("/")
            if "*" in clean or "<" in clean:
                continue
            # Ignore ephemeral python package and frontend build targets
            if (
                clean in EPHEMERAL_BUILD_TARGETS
                or clean.startswith("frontend/.next/")
                or clean.startswith("frontend/node_modules/")
                or clean.startswith("sdk/python/dist/")
                or clean.startswith("sdk/python/build/")
            ):
                continue
            # Ignore historical deleted artifacts cited in audit remediation records
            if clean in REMEDIATION_DELETED_ITEMS:
                continue
            if clean in checked:
                continue
            checked.add(clean)
            full = os.path.join(ROOT_DIR, clean.replace("/", os.sep))
            if not os.path.exists(full):
                # Rust module path compatibility: foo.rs can be represented as foo/mod.rs
                mod_rs = os.path.join(ROOT_DIR, clean[:-3].replace("/", os.sep), "mod.rs") if clean.endswith(".rs") else ""
                if mod_rs and os.path.exists(mod_rs):
                    pass
                else:
                    missing.add(clean)

    print(f"Verified {len(checked)} distinct source paths referenced across Markdown.")
    if missing:
        print("WARNINGS: referenced source paths not found:")
        for item in sorted(missing):
            print(f"  - {item}")
        if strict:
            print("FAILED (--strict): missing paths above.")
            return 1
        print("Advisory mode: not failing the build.")
        return 0

    print("OK: all referenced source paths exist on disk.")
    return 0


if __name__ == "__main__":
    sys.exit(verify("--strict" in sys.argv[1:]))
