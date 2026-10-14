# Sunborn Data, Research & Reproducibility Pipeline

# Data and Research Pipeline

## Dataset flow

Source → provenance/license check → normalization → labeling → train/dev/test or benchmark split → DVC version → benchmark package → immutable artifact store.

## Experiment flow

Linear research issue → experiment manifest → Git commit → DVC dataset version → model/embedding/reranker versions → config + seed + hardware → run → MLflow metrics/artifacts → statistical analysis → result label → conclusion → ADR/implementation decision.

## Benchmark flow

Workload definition → baseline B0–B6 → deterministic runner → raw results → validation → statistics → comparison → report → paper figure/table.

## Security research flow

Threat → attack fixture → exploit measurement → mitigation → regression benchmark → retest → security conclusion.

## Required provenance

commit SHA
dataset version
model/provider version
embedding version
reranker version
policy version
config
seed
hardware/software environment
run ID
raw artifact hash

## Reproducibility command contract

run-baselines
run-full
run-ablation
run-security
run-scale
Each command must resolve exact versions and produce deterministic manifests plus machine-readable output where feasible.
