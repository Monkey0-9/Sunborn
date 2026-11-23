PYTHON ?= python3
ifeq ($(OS),Windows_NT)
PYTHON := python
endif

.PHONY: all build test check fmt lint run clean help docker-up docker-down brokers dev test-frontend build-frontend test-sdk lint-helm verify-stack reproduce verify-all doctor config-check

all: check test

help:
	@echo "Sunborn Engineering Commands:"
	@echo "  make build         - Build all crates in release mode"
	@echo "  make test          - Run all unit and integration tests"
	@echo "  make check         - Type check all workspace crates"
	@echo "  make fmt           - Format all code with rustfmt"
	@echo "  make lint          - Run clippy with strict warnings"
	@echo "  make test-frontend - Run frontend test suite (BFF + E2E)"
	@echo "  make build-frontend- Build Next.js production bundle"
	@echo "  make test-sdk      - Run Python SDK test suites (sync + async)"
	@echo "  make lint-helm     - Run Helm chart lint and schema checks"
	@echo "  make verify-stack  - Run multi-container live stack pre-flight"
	@echo "  make reproduce     - Run research reproducibility suite"
	@echo "  make verify-all    - Comprehensive pre-merge gate validation"
	@echo "  make doctor        - Pre-flight: validate config, storage, port, key material"
	@echo "  make config-check  - Validate configuration and print the redacted effective view"
	@echo "  make run           - Run the sunborn-gateway service"
	@echo "  make brokers       - Start Postgres, Dragonfly and Qdrant only"
	@echo "  make dev           - Wire brokers, generate .env.local, and run the gateway"
	@echo "  make docker-up     - Start local dependencies via Docker Compose"
	@echo "  make docker-down   - Stop local dependencies"

build:
	cargo build --release

test:
	cargo test --all-targets --all-features

test-frontend:
	npm test --prefix frontend

build-frontend:
	npm run build --prefix frontend

test-sdk:
	$(PYTHON) -m unittest sdk/python/test_sunborn_agent.py
	$(PYTHON) -m unittest sdk/python/tests/test_sdk_package.py

lint-helm:
	helm lint deploy/helm/sunborn --set secrets.existingSecret="ci-validation"

verify-stack:
	bash deploy/verify_live_stack.sh

reproduce:
	bash research/reproduce.sh

verify-all: check test test-frontend build-frontend test-sdk lint-helm verify-stack reproduce

check:
	cargo check --workspace

fmt:
	cargo fmt --all

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

doctor:
	cargo run -p sunborn-gateway -- doctor

config-check:
	cargo run -p sunborn-gateway -- config-check

run:
	cargo run -p sunborn-gateway

brokers:
	docker compose -f deploy/docker-compose.yml up -d postgres dragonfly qdrant

dev:
	bash scripts/dev-up.sh

docker-up:
	docker compose -f deploy/docker-compose.yml up -d

docker-down:
	docker compose -f deploy/docker-compose.yml down

clean:
	cargo clean
