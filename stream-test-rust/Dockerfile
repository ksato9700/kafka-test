FROM rust:1.76-slim-bookworm AS builder

RUN apt-get update && apt-get install -y 
    cmake 
    build-essential 
    pkg-config 
    libssl-dev 
    zlib1g-dev 
    libsnappy-dev 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY stream-test-rust/Cargo.toml stream-test-rust/Cargo.lock ./
# Create dummy main to pre-build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY stream-test-rust/src ./src
COPY schemas /app/schemas
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y 
    libssl3 
    zlib1g 
    libsnappy1v5 
    ca-certificates 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/processor /app/stream-test-rust
COPY --from=builder /app/schemas /app/schemas

CMD ["./stream-test-rust"]
