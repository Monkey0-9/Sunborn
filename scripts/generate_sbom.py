#!/usr/bin/env python3
"""
Sunborn Software Bill of Materials (SBOM) Generator.

Produces a deterministic CycloneDX v1.5 JSON SBOM from Cargo.lock and the
frontend package.json. Output is byte-stable for identical inputs: components
are sorted, the serial number is derived from the component digest, and the
timestamp is only emitted when SOURCE_DATE_EPOCH is set.
"""
import hashlib
import json
import os
import uuid
from datetime import datetime, timezone
from pathlib import Path


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


def generate_sbom():
    components = collect_components()

    # Deterministic serial derived from the component set.
    digest = hashlib.sha256(
        json.dumps(components, sort_keys=True, separators=(",", ":")).encode("utf-8")
    ).hexdigest()
    serial = uuid.uuid5(uuid.NAMESPACE_URL, f"sunborn-sbom:{digest}")

    metadata = {
        "component": {
            "type": "application",
            "name": "sunborn",
            "version": "0.1.0",
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

    out_file = Path("sbom.cyclonedx.json")
    out_file.write_text(json.dumps(sbom, indent=2) + "\n", encoding="utf-8")
    print(
        f"OK: deterministic CycloneDX v1.5 SBOM written to {out_file.name} "
        f"({len(components)} components, sha256={digest[:12]})"
    )
    return len(components)


if __name__ == "__main__":
    generate_sbom()
