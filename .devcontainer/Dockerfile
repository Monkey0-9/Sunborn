FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# Install Node.js 20.x, Python 3.11, PostgreSQL client, and build essentials
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
       curl \
       pkg-config \
       libssl-dev \
       postgresql-client \
       python3-pip \
       python3-dev \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Install cargo-audit and cargo-tarpaulin as vscode user
USER vscode
RUN cargo install cargo-audit --locked
