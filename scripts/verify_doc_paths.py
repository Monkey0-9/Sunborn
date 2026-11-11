#!/usr/bin/env python3
"""
Sunborn Documentation Governance: Path & Implementation Verifier
Ensures all source code and test paths referenced in docs/ actually exist in the repository.
"""

import os
import re
import sys

ROOT_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def verify_feature_matrix_paths() -> int:
    matrix_file = os.path.join(ROOT_DIR, "docs", "audit", "FEATURE_VERIFICATION_MATRIX.md")
    if not os.path.exists(matrix_file):
        print(f"Error: {matrix_file} not found")
        return 1

    with open(matrix_file, "r", encoding="utf-8") as f:
        content = f.read()

    # Match paths in code ticks like `crates/sunborn-cache/src/memory.rs`
    path_pattern = re.compile(r"`((?:crates|apps|frontend|deploy|research|sdk|scripts|migrations)/[a-zA-Z0-9_\-\./]+)`")
    matches = path_pattern.findall(content)

    missing = []
    checked = set()

    for path in matches:
        # Ignore directory references or wildcards
        clean_path = path.rstrip("/")
        if "*" in clean_path or clean_path.endswith(".sql") and "0001-0003" in clean_path:
            continue
        if clean_path in checked:
            continue
        checked.add(clean_path)

        full_path = os.path.join(ROOT_DIR, clean_path.replace("/", os.sep))
        if not os.path.exists(full_path):
            missing.append(clean_path)

    print(f"Verified {len(checked)} distinct implementation paths in FEATURE_VERIFICATION_MATRIX.md")
    if missing:
        print("FAILED: The following referenced paths do not exist:")
        for m in missing:
            print(f"  - {m}")
        return 1

    print("✓ All referenced source and test paths exist on disk.")
    return 0


if __name__ == "__main__":
    sys.exit(verify_feature_matrix_paths())
