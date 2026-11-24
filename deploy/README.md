# Sunborn Deployment Manifests (`deploy/`)

Production and staging infrastructure manifests for deploying Sunborn across Docker, Kubernetes, and cloud environments.

## Manifest Structure

- **`docker-compose.yml`**: Local development setup including Sunborn Gateway, PostgreSQL, DragonflyDB, and Qdrant.
- **`docker-compose.prod.yml`**: Hardened production Compose configuration with KMS integration and persistent storage.
- **`docker-compose.distributed-test.yml`**: Containerized multi-replica distributed invalidation test harness setup.
- **[`helm/`](./helm/)**: Helm charts for multi-pod Kubernetes deployment with horizontal pod autoscaling (HPA) and Prometheus integration.
- **[`k8s/`](./k8s/)**: Standalone Kubernetes manifests, ConfigMaps, Secret templates, and ServiceDefinitions.
- **[`grafana/`](./grafana/)**: Pre-configured Grafana telemetry dashboard templates for gateway observability.

## Quickstart

```bash
# Start full local stack
docker compose up -d
```
