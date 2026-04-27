
# Dockerfile for CrabJar AI Agent
# Uses Redox OS and Nushell as default environment

FROM rust:1.85-alpine AS builder

# Install dependencies
RUN apk add --no-cache \
    build-base \
    git \
    curl \
    wasm-pack \
    wasmer \
    && rm -rf /var/cache/apk/*

WORKDIR /app

# Copy all files to the container
COPY . .

# Build the WASM tools first
RUN ./agent-wasm/tools/build.sh || true  # Continue even if build fails

# Build the agent
RUN ./build.sh

FROM redoxos/latest AS runtime

# Install Nushell as default shell
RUN nu --version || (echo "Installing Nushell..." && \
    curl https://getnu.sh | sh)

COPY --from=builder /app/target/release/crabjar /usr/local/bin/crabjar

WORKDIR /workspace

ENTRYPOINT ["/usr/local/bin/crabjar"]

CMD ["--repl"]
