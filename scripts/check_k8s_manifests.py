#!/usr/bin/env python3
"""Check repository Kubernetes manifests for deployable placeholder secrets."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
K8S = ROOT / "deploy" / "k8s"


def main() -> int:
    kustomization = (K8S / "kustomization.yaml").read_text()
    secret_template = (K8S / "secrets.yaml").read_text()
    frontend = (K8S / "frontend-deployment.yaml").read_text()

    errors = []
    if re.search(r"(?m)^\s*-\s*secrets\.yaml\s*$", kustomization):
        errors.append("deployable Kustomize resources include secrets.yaml")
    if re.search(r"(?m)^\s*kind:\s*Secret\s*$", secret_template):
        errors.append("deploy/k8s/secrets.yaml must not define a deployable Secret")
    if "secretRef:" in frontend or "secretKeyRef:" in frontend:
        errors.append("frontend deployment must not receive gateway credentials")

    placeholder_pattern = re.compile(
        r"CHANGE_ME|change_me|change-in-vault|change_in_vault|placeholder|replace-me",
        re.IGNORECASE,
    )
    for path in K8S.glob("*.yaml"):
        for line_number, line in enumerate(path.read_text().splitlines(), start=1):
            if line.lstrip().startswith("#"):
                continue
            if placeholder_pattern.search(line):
                errors.append(f"{path.relative_to(ROOT)}:{line_number}: placeholder in deployable manifest")

    if errors:
        print("Kubernetes manifest security check failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("Kubernetes manifests contain no deployable placeholder credentials or frontend gateway-secret references.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
