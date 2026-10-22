#!/usr/bin/env python3
"""Check gateway route/method parity and minimum OpenAPI operation structure.

This verifies source route declarations against documented OpenAPI operations, plus
presence of response, security, and any declared request-body schemas. It does not
prove that Rust runtime validation/auth behavior matches those schemas; that still
requires contract and authorization tests.
"""

import os
import re
import sys

try:
    import yaml
except ImportError:
    yaml = None

def route_operations(source):
    operations = set()
    route_start = re.compile(r"\.route\s*\(")
    method_call = re.compile(r"(?<![A-Za-z0-9_])(?:axum::routing::)?(get|post|put|patch|delete|head|options)\s*\(")
    for match in route_start.finditer(source):
        open_paren = source.find("(", match.start())
        depth = 0
        quoted = False
        escaped = False
        end = None
        for index in range(open_paren, len(source)):
            char = source[index]
            if quoted:
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    quoted = False
                continue
            if char == '"':
                quoted = True
            elif char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
                if depth == 0:
                    end = index
                    break
        if end is None:
            raise ValueError(f"Unclosed Router::route call at source offset {match.start()}")

        block = source[open_paren + 1:end]
        path_match = re.match(r'\s*"([^\"]+)"\s*,', block)
        if not path_match:
            raise ValueError(f"Could not parse route path at source offset {match.start()}")
        route = path_match.group(1).replace("{*key}", "{key}")
        methods = {item.group(1).lower() for item in method_call.finditer(block[path_match.end():])}
        if not methods:
            raise ValueError(f"No HTTP methods parsed for {route}")
        operations.update((route, method) for method in methods)
    return operations


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

    try:
        gateway_operations = route_operations(lib_content)
    except ValueError as error:
        print(f"ERROR: {error}")
        sys.exit(1)
    if not gateway_operations:
        print(f"ERROR: Could not parse any routes from {lib_path}")
        sys.exit(1)

    # 3. Parse OpenAPI specification
    with open(openapi_doc_path, "r", encoding="utf-8") as f:
        doc_content = f.read()

    if yaml is None:
        print("ERROR: PyYAML is required for method and OpenAPI operation-structure validation")
        sys.exit(1)
    try:
        spec = yaml.safe_load(doc_content)
    except yaml.YAMLError as error:
        print(f"ERROR: Invalid OpenAPI YAML: {error}")
        sys.exit(1)

    paths = spec.get("paths", {})
    openapi_operations = set()
    structural_errors = []
    valid_methods = {"get", "post", "put", "patch", "delete", "head", "options", "trace"}
    global_security = spec.get("security")
    for route, path_item in paths.items():
        if not isinstance(path_item, dict):
            structural_errors.append(f"{route}: path item must be an object")
            continue
        normalized_route = route
        for method, operation in path_item.items():
            if method.lower() not in valid_methods:
                continue
            openapi_operations.add((normalized_route, method.lower()))
            label = f"{method.upper()} {route}"
            if not isinstance(operation, dict):
                structural_errors.append(f"{label}: operation must be an object")
                continue
            responses = operation.get("responses")
            if not isinstance(responses, dict) or not responses:
                structural_errors.append(f"{label}: responses must be a non-empty object")
            else:
                for code, response in responses.items():
                    if not isinstance(response, dict) or not (response.get("description") or response.get("$ref")):
                        structural_errors.append(f"{label}: response {code} needs a description or $ref")
                if not any(str(code).startswith("2") or str(code) == "default" for code in responses):
                    structural_errors.append(f"{label}: no documented successful response")
            if "security" not in operation and global_security is None:
                structural_errors.append(f"{label}: security must be explicit or inherited from the document")
            request_body = operation.get("requestBody")
            if request_body is not None:
                if not isinstance(request_body, dict) or not isinstance(request_body.get("content"), dict):
                    structural_errors.append(f"{label}: requestBody must define content")
                elif not any(
                    isinstance(media, dict) and isinstance(media.get("schema"), dict)
                    for media in request_body["content"].values()
                ):
                    structural_errors.append(f"{label}: requestBody content must define a schema")

    # 4. Compare (path, HTTP method) pairs.
    missing_in_spec = gateway_operations - openapi_operations
    undocumented_in_gateway = openapi_operations - gateway_operations

    has_error = False
    if missing_in_spec:
        print("❌ OpenAPI Drift Error: The following gateway routes are MISSING in openapi.yaml:")
        for route, method in sorted(missing_in_spec):
            print(f"   - {method.upper()} {route}")
        has_error = True

    if undocumented_in_gateway:
        print("⚠️ Warning: The following openapi.yaml paths are not in gateway lib.rs:")
        for route, method in sorted(undocumented_in_gateway):
            print(f"   - {method.upper()} {route}")
        has_error = True

    if structural_errors:
        print("❌ OpenAPI operation contract errors:")
        for error in structural_errors:
            print(f"   - {error}")
        has_error = True

    if has_error:
        print(f"\nTotal Gateway Operations : {len(gateway_operations)}")
        print(f"Total OpenAPI Operations : {len(openapi_operations)}")
        sys.exit(1)

    print(f"✓ All {len(gateway_operations)} gateway method/path operations match OpenAPI and pass minimum structure checks.")
    print("! Runtime schema, authorization, and error-behavior parity still require contract and policy tests.")
    print(f"✓ Root openapi.yaml verified reachable at {openapi_root_path}")
    sys.exit(0)

if __name__ == "__main__":
    main()
