#!/usr/bin/env python3
"""
Sunborn CI Quality Gate: OpenAPI Specification Drift Validator
Verifies that all HTTP routes defined in crates/sunborn-gateway/src/lib.rs
are completely and strictly documented in docs/api/openapi.yaml and openapi.yaml.
"""

import os
import re
import sys

try:
    import yaml
except ImportError:
    yaml = None

def main():
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    lib_path = os.path.join(root_dir, "crates", "sunborn-gateway", "src", "lib.rs")
    openapi_doc_path = os.path.join(root_dir, "docs", "api", "openapi.yaml")
    openapi_root_path = os.path.join(root_dir, "openapi.yaml")

    # 1. Check root openapi.yaml existence
    if not os.path.exists(openapi_root_path):
        print("ERROR: Root openapi.yaml does not exist! Expected symlink to docs/api/openapi.yaml")
        sys.exit(1)

    # 2. Extract routes from lib.rs
    with open(lib_path, "r", encoding="utf-8") as f:
        lib_content = f.read()

    raw_routes = re.findall(r'\.route\(\s*"([^"]+)"', lib_content)
    if not raw_routes:
        print(f"ERROR: Could not parse any routes from {lib_path}")
        sys.exit(1)

    # Normalize wildcard syntax (e.g. /v1/caches/{*key} -> /v1/caches/{key})
    normalized_gateway_routes = set()
    for r in raw_routes:
        norm = r.replace("{*key}", "{key}")
        normalized_gateway_routes.add(norm)

    # 3. Parse OpenAPI specification
    with open(openapi_doc_path, "r", encoding="utf-8") as f:
        doc_content = f.read()

    if yaml is not None:
        spec = yaml.safe_load(doc_content)
        openapi_paths = set(spec.get("paths", {}).keys())
    else:
        # Fallback to stdlib regex extraction if PyYAML is not installed in the environment
        paths_match = re.search(r'\npaths:\n(.*?)(?:\ncomponents:|\Z)', doc_content, re.DOTALL)
        if paths_match:
            openapi_paths = set(re.findall(r'^\s{2}(/[^\s:]+):', paths_match.group(1), re.MULTILINE))
        else:
            print("ERROR: Could not find paths section in openapi.yaml")
            sys.exit(1)

    # 4. Compare sets
    missing_in_spec = normalized_gateway_routes - openapi_paths
    undocumented_in_gateway = openapi_paths - normalized_gateway_routes

    has_error = False
    if missing_in_spec:
        print("❌ OpenAPI Drift Error: The following gateway routes are MISSING in openapi.yaml:")
        for r in sorted(missing_in_spec):
            print(f"   - {r}")
        has_error = True

    if undocumented_in_gateway:
        print("⚠️ Warning: The following openapi.yaml paths are not in gateway lib.rs:")
        for r in sorted(undocumented_in_gateway):
            print(f"   - {r}")
        has_error = True

    if has_error:
        print(f"\nTotal Gateway Routes : {len(normalized_gateway_routes)}")
        print(f"Total Documented Paths: {len(openapi_paths)}")
        sys.exit(1)

    print(f"✓ All {len(normalized_gateway_routes)} gateway routes strictly match openapi.yaml (zero drift).")
    print(f"✓ Root openapi.yaml verified reachable at {openapi_root_path}")
    sys.exit(0)

if __name__ == "__main__":
    main()
