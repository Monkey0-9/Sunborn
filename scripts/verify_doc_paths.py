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
        for name in filenames:
            if name.endswith(".md"):
                files.append(os.path.join(dirpath, name))
    return files


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
            if clean in checked:
                continue
            checked.add(clean)
            full = os.path.join(ROOT_DIR, clean.replace("/", os.sep))
            if not os.path.exists(full):
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
