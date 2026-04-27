FROM rust:1.85

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        pkg-config \
        build-essential \
        clang \
        lld \
        libssl-dev \
        sqlite3 \
        libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Nushell
RUN curl -fsSL https://getnu.sh | sh

WORKDIR /workspace

CMD ["/bin/bash"]
