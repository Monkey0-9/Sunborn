.PHONY: all build test check fmt lint run clean help docker-up docker-down

all: check test

help:
	@echo "Sunborn Engineering Commands:"
	@echo "  make build       - Build all crates in release mode"
	@echo "  make test        - Run all unit and integration tests"
	@echo "  make check       - Type check all workspace crates"
	@echo "  make fmt         - Format all code with rustfmt"
	@echo "  make lint        - Run clippy with strict warnings"
	@echo "  make run         - Run the sunborn-gateway service"
	@echo "  make docker-up   - Start local dependencies via Docker Compose"
	@echo "  make docker-down - Stop local dependencies"

build:
	cargo build --release

test:
	cargo test --all-targets --all-features

check:
	cargo check --workspace

fmt:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

run:
	cargo run -p sunborn-gateway

docker-up:
	docker compose -f deploy/docker-compose.yml up -d

docker-down:
	docker compose -f deploy/docker-compose.yml down

clean:
	cargo clean
