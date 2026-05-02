# Design: kafka-test-krafka

**Date:** 2026-05-02  
**Goal:** Add a `kafka-test-krafka/` implementation using the pure-Rust `krafka` crate, achieving full feature parity with `kafka-test-rust` (rdkafka) while documenting build and API differences between the two libraries.

---

## 1. Directory Structure

```
kafka-test-krafka/
├── src/
│   ├── producer.rs
│   └── consumer.rs
├── Cargo.toml
├── Makefile
├── Dockerfile
├── .dockerignore
├── .gitignore
└── README.md
```

Two binaries (`kafka-producer`, `kafka-consumer`) built from the same Cargo crate — identical layout to `kafka-test-rust`.

---

## 2. Dependencies

```toml
[dependencies]
krafka = "0.7"
apache-avro = "0.21"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
serde_json = "1.0"
```

No `rdkafka`, `futures`, cmake, or C toolchain required.

**Minimum Kafka broker version:** 3.9+ (required by krafka; `apache/kafka:latest` satisfies this).

---

## 3. API Comparison vs kafka-test-rust

| Concern | `kafka-test-rust` (rdkafka) | `kafka-test-krafka` (krafka) |
|---|---|---|
| Producer API | `FutureProducer` + `FutureRecord` | `ProducerBuilder` + `ProducerRecord` |
| Consumer API | `StreamConsumer` + stream | `ConsumerBuilder` + `consumer.poll()` loop |
| IPv4 forcing | `broker.address.family = v4` config key | Use `127.0.0.1` directly; Happy Eyeballs (RFC 8305) for Docker |
| Build deps (builder stage) | cmake, g++, openssl-dev, zlib-dev, sasl-dev, curl-dev + statics | `musl-dev` only |
| Runtime deps | libgcc, libstdc++ | none |
| Min Kafka version | Any | 3.9+ |

---

## 4. Producer Design

1. Load Avro schema from paths: `../schemas/message.avsc` → `/app/schemas/message.avsc` → `schemas/message.avsc`
2. Read `KAFKA_BOOTSTRAP_SERVERS` (default `127.0.0.1:9094`) and `TOPIC_NAME` (default `my-topic-1`) from env
3. Build producer: `ProducerBuilder::new(vec![bootstrap_servers]).client_id("kafka-test-krafka-producer").build().await`
4. Loop every 2 seconds:
   - Capture `event_time` as Unix seconds (f64)
   - Build Avro record: `message_id` (long), `event_time` (double), `content` (string)
   - Serialize with `apache_avro::to_avro_datum` (raw binary, no OCF header)
   - Send: `ProducerRecord::new(topic).value(&payload)`
   - Log: `🚀 Sent: Message {id} ({n} bytes)`
5. On SIGINT: `producer.flush().await`, then break

---

## 5. Consumer Design

1. Load Avro schema (same path fallback as producer)
2. Read `KAFKA_BOOTSTRAP_SERVERS`, `TOPIC_NAME`, `CONSUMER_GROUP_ID` (default `my-group-1`), `AUTO_OFFSET_RESET` (default `latest`) from env
3. Build consumer: `ConsumerBuilder::new(vec![bootstrap_servers]).group_id(...).auto_offset_reset(...).build().await`
4. `consumer.subscribe(["topic"])`
5. Poll loop (`tokio::select!` on `consumer.poll(100ms)` and `ctrl_c()`):
   - Deserialize raw Avro bytes with `apache_avro::from_avro_datum`
   - Capture `received_time`, compute `latency = received_time - event_time`
   - Log: `📨 New message: [ID={id}] {content} at {event_time:.3}`
   - Log: `⏱️ Latency: {latency:.3} seconds`
6. On SIGINT: commit offsets, then break

---

## 6. Dockerfile

```dockerfile
FROM rust:1-alpine3.23 AS builder
WORKDIR /usr/src/app
RUN apk add --no-cache musl-dev
COPY kafka-test-krafka .
RUN cargo build --release

FROM alpine:3.23
WORKDIR /app
COPY schemas /app/schemas
COPY --from=builder /usr/src/app/target/release/kafka-producer /usr/local/bin/kafka-producer
COPY --from=builder /usr/src/app/target/release/kafka-consumer /usr/local/bin/kafka-consumer
CMD ["kafka-consumer"]
```

Multi-stage, Alpine-based. Builder stage requires only `musl-dev` — no C++ toolchain — because krafka is pure Rust. Runtime stage has zero extra dependencies.

---

## 7. Makefile Targets

Standard targets matching all other `kafka-test-*` implementations:

| Target | Description |
|---|---|
| `run-consumer` | `cargo run --bin kafka-consumer` |
| `run-producer` | `cargo run --bin kafka-producer` |
| `docker-build` | `cd .. && docker build -f kafka-test-krafka/Dockerfile -t kafka-test-krafka .` (repo root as context so `schemas/` is accessible) |
| `docker-run-consumer` | Run consumer in `kafka-net` with `kafka-broker:9092` |
| `docker-run-producer` | Run producer in `kafka-net` with `kafka-broker:9092` |

---

## 8. Parity Requirements

Per `AGENTS.md`, the following must match all other implementations:

- Log format: `📨 New message: [ID=...] ...` and `⏱️ Latency: ...`
- Default bootstrap: `127.0.0.1:9094` (local), `kafka-broker:9092` (Docker)
- `event_time` transmitted as **seconds** (f64/double), not milliseconds
- Graceful SIGINT handling with offset commit
