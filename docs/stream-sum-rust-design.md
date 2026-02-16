# Design Document: High-Performance Rust Streaming Sum Processor

## 1. Objective
To implement the `streaming-performance-spec.md` in Rust, aiming to exceed the Java implementation's throughput by utilizing Rust's zero-cost abstractions and efficient memory management.

## 2. Architecture & Technology Stack
- **Language**: Rust (Edition 2021)
- **Kafka Library**: `rust-rdkafka` (based on `librdkafka`)
- **Serialization**: `apache-avro` (with manual optimization for object reuse)
- **Async Runtime**: `tokio`
- **Concurrency Model**: 
    - **Multi-Consumer/Producer Strategy**: Use a pool of independent worker tasks.
    - **Pipelining**: Decouple consumption, processing, and production using bounded MPSC channels to prevent head-of-line blocking.

## 3. High-Performance Optimizations

### Zero-Allocation Serialization
- **Buffer Reuse**: Utilize a `ThreadLocal` or a pool of `Vec<u8>` to reuse allocation for Avro encoding/decoding.
- **Fast Avro**: Use the `apache-avro` "Raw" API to avoid unnecessary intermediate generic mappings where possible.

### Kafka Tuning
- **`librdkafka` config**:
    - `batch.num.messages`: 100,000
    - `queue.buffering.max.ms`: 20
    - `compression.codec`: `snappy`
    - `socket.nagle.disable`: `true`
    - `fetch.queue.backoff.ms`: 1

### Concurrency
- **Worker Tasks**: Each Kafka partition will be handled by a dedicated Tokio task to ensure parallel processing without lock contention.
- **Lock-free Counters**: Use `std::sync::atomic::AtomicU64` for throughput monitoring.

## 4. Project Structure
`stream-test-rust/`
├── Cargo.toml            # Dependencies: rdkafka, avro-rs, tokio, serde
├── Makefile              # Commands: build, run-stream, docker-run
├── Dockerfile            # Optimized Alpine/Distroless build
└── src/
    ├── main.rs           # Entry point, config, and throughput reporter
    ├── processor.rs      # Stream processing loop logic
    └── avro_util.rs      # Optimized Avro SerDe with buffer reuse

## 5. Throughput Monitoring
Identical to the Java implementation:
- Periodic task (every 5s) calculates `total_messages / 5`.
- Result is logged to stdout.
