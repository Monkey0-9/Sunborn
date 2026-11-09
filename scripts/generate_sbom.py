#!/usr/bin/env python3
"""
Sunborn Software Bill of Materials (SBOM) Generator (DEF-021)
Generates an enterprise-compliant CycloneDX v1.5 JSON SBOM across Rust and Frontend dependencies.
"""
import json
import uuid
from datetime import datetime, timezone
from pathlib import Path

def generate_sbom():
    cargo_lock = Path("Cargo.lock")
    components = []
    
    if cargo_lock.exists():
        lines = cargo_lock.read_text(encoding="utf-8").splitlines()
        curr_name, curr_ver = None, None
        for line in lines:
            line = line.strip()
            if line.startswith("name = "):
                curr_name = line.split('"')[1]
            elif line.startswith("version = ") and curr_name:
                curr_ver = line.split('"')[1]
                components.append({
                    "type": "library",
                    "bom-ref": f"pkg:cargo/{curr_name}@{curr_ver}",
                    "name": curr_name,
                    "version": curr_ver,
                    "purl": f"pkg:cargo/{curr_name}@{curr_ver}",
                    "scope": "required"
                })
                curr_name, curr_ver = None, None

    # Frontend npm dependencies
    pkg_json = Path("frontend/package.json")
    if pkg_json.exists():
        data = json.loads(pkg_json.read_text(encoding="utf-8"))
        for name, ver in data.get("dependencies", {}).items():
            clean_ver = ver.lstrip("^~")
            components.append({
                "type": "library",
                "bom-ref": f"pkg:npm/{name}@{clean_ver}",
                "name": name,
                "version": clean_ver,
                "purl": f"pkg:npm/{name}@{clean_ver}",
                "scope": "required"
            })

    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": f"urn:uuid:{uuid.uuid4()}",
        "version": 1,
        "metadata": {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "component": {
                "type": "application",
                "name": "sunborn",
                "version": "0.1.0",
                "description": "Autonomic Semantic Cache & LLM Invalidation Gateway"
            }
        },
        "components": components
    }

    out_file = Path("sbom.cyclonedx.json")
    out_file.write_text(json.dumps(sbom, indent=2), encoding="utf-8")
    print(f"[✓] CycloneDX v1.5 SBOM generated successfully: {out_file.name} ({len(components)} components)")
    return len(components)

if __name__ == "__main__":
    generate_sbom()
