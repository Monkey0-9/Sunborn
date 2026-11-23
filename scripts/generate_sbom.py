#!/usr/bin/env python3
"""
Sunborn Software Bill of Materials (SBOM) Generator.

Produces a deterministic CycloneDX v1.5 JSON SBOM from Cargo.lock and the
frontend package.json. Output is byte-stable for identical inputs: components
are sorted, the serial number is derived from the component digest, and the
timestamp is only emitted when SOURCE_DATE_EPOCH is set.

Usage:
    python scripts/generate_sbom.py            # write/refresh sbom.cyclonedx.json
    python scripts/generate_sbom.py --check    # fail if the file is missing or stale
"""
import argparse
import hashlib
import json
import os
import re
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path

DEFAULT_OUTPUT = "sbom.cyclonedx.json"


def _workspace_version() -> str:
    """Read the workspace version so the SBOM metadata cannot drift from Cargo.toml."""
    cargo_toml = Path("Cargo.toml")
    if not cargo_toml.exists():
        return "0.0.0"
    match = re.search(
        r'^\[workspace\.package\][\s\S]*?^\s*version\s*=\s*"([^"]+)"',
        cargo_toml.read_text(encoding="utf-8"),
        re.MULTILINE,
    )
    return match.group(1) if match else "0.0.0"


def collect_components():
    components = []

    cargo_lock = Path("Cargo.lock")
    if cargo_lock.exists():
        lines = cargo_lock.read_text(encoding="utf-8").splitlines()
        curr_name, curr_ver = None, None
        for line in lines:
            line = line.strip()
            if line.startswith("name = "):
                curr_name = line.split('"')[1]
            elif line.startswith("version = ") and curr_name:
                curr_ver = line.split('"')[1]
                components.append(
                    {
                        "type": "library",
                        "bom-ref": f"pkg:cargo/{curr_name}@{curr_ver}",
                        "name": curr_name,
                        "version": curr_ver,
                        "purl": f"pkg:cargo/{curr_name}@{curr_ver}",
                        "scope": "required",
                    }
                )
                curr_name, curr_ver = None, None

    pkg_json = Path("frontend/package.json")
    if pkg_json.exists():
        data = json.loads(pkg_json.read_text(encoding="utf-8"))
        for name, ver in data.get("dependencies", {}).items():
            clean_ver = ver.lstrip("^~")
            components.append(
                {
                    "type": "library",
                    "bom-ref": f"pkg:npm/{name}@{clean_ver}",
                    "name": name,
                    "version": clean_ver,
                    "purl": f"pkg:npm/{name}@{clean_ver}",
                    "scope": "required",
                }
            )

    # Deterministic order and dedupe by bom-ref.
    unique = {}
    for component in components:
        unique[component["bom-ref"]] = component
    return [unique[ref] for ref in sorted(unique)]


def render_sbom(components) -> tuple[str, str]:
    """Return (deterministic JSON text, component digest)."""
    # Deterministic serial derived from the component set.
    digest = hashlib.sha256(
        json.dumps(components, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    serial = uuid.uuid5(uuid.NAMESPACE_URL, f"sunborn-sbom:{digest}")

    metadata = {
        "component": {
            "type": "application",
            "name": "sunborn",
            "version": _workspace_version(),
            "description": "Autonomic Semantic Cache & LLM Invalidation Gateway",
        }
    }
    # Timestamp is optional in CycloneDX; only emit it when explicitly requested so
    # regeneration does not create spurious diffs.
    source_date_epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if source_date_epoch and source_date_epoch.isdigit():
        metadata["timestamp"] = datetime.fromtimestamp(
            int(source_date_epoch), tz=timezone.utc
        ).isoformat()

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{serial}",
        "version": 1,
        "metadata": metadata,
        "components": components,
    }
    return json.dumps(sbom, indent=2) + "\n", digest


def _status(prefix: str, out_file: Path, count: int, digest: str) -> None:
    print(
        f"{prefix}: deterministic CycloneDX v1.5 SBOM at {out_file.name} "
        f"({count} components, sha256={digest[:12]})"
    )


def generate_sbom(output: str = DEFAULT_OUTPUT, check: bool = False) -> int:
    components = collect_components()
    sbom_text, digest = render_sbom(components)
    out_file = Path(output)

    if check:
        if not out_file.exists():
            print(
                f"ERROR: {out_file.name} is missing; run `python scripts/generate_sbom.py`.",
                file=sys.stderr,
            )
            return 1
        try:
            current = out_file.read_text(encoding="utf-8")
        except OSError as exc:
            print(f"ERROR: cannot read {out_file.name}: {exc}", file=sys.stderr)
            return 1
        if current != sbom_text:
            print(
                f"ERROR: {out_file.name} is stale; run `python scripts/generate_sbom.py` "
                "and commit the regenerated artifact.",
                file=sys.stderr,
            )
            return 1
        _status("OK (verified)", out_file, len(components), digest)
        return 0

    if out_file.exists():
        try:
            if out_file.read_text(encoding="utf-8") == sbom_text:
                _status("OK (unchanged)", out_file, len(components), digest)
                return 0
        except OSError as exc:
            print(
                f"WARN: cannot read existing {out_file.name} ({exc}); rewriting.",
                file=sys.stderr,
            )

    out_file.write_text(sbom_text, encoding="utf-8")
    _status("OK (written)", out_file, len(components), digest)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check",
        action="store_true",
        help="verify the checked-in SBOM is present and current; do not write",
    )
    parser.add_argument("--output", default=DEFAULT_OUTPUT, help="SBOM output path")
    args = parser.parse_args()
    return generate_sbom(output=args.output, check=args.check)


if __name__ == "__main__":
    sys.exit(main())
