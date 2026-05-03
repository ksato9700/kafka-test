# Design: stream-test-krafka

**Date:** 2026-05-03  
**Goal:** Add a `stream-test-krafka/` benchmark using the pure-Rust `krafka` crate (no C deps), following the same pattern as `stream-test-rust`. Measure throughput and compare with existing results: Java ~6.3M msg/sec, C ~4.5M msg/sec, Rust (rdkafka) ~3.5M msg/sec, Go ~2.4M msg/sec.

---

## 1. Directory Structure

```
stream-test-krafka/
├── src/bin/
│   ├── processor.rs   ← main benchmark binary
│   ├── producer.rs    ← continuous data generator
│   └── consumer.rs    ← result printer
├── Cargo.toml
├── Makefile
├── Dockerfile
├── .dockerignore
├── .gitignore
└── README.md
```

Three binaries in `src/bin/`. `processor` is the benchmark entry point; `producer` and `consumer` are development utilities only.

---

## 2. Dependencies

```toml
[dependencies]
krafka = "0.7"
tokio = { version = "1", features = ["rt", "rt-multi-thread"] }
rand = { version = "0.8", features = ["small_rng"] }
log = "0.4"
env_logger = "0.11"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

No rdkafka, no apache-avro, no serde, no futures, no lazy_static. `tokio` uses minimal features (`rt` + `rt-multi-thread`) — not `full`.

**Minimum Kafka broker version:** 3.9+ (required by krafka; `apache/kafka:latest` satisfies this).

---

## 3. Message Encoding

**Zig-Zag LEB128** (same as all other stream-test implementations):

- Input record: `[count varint][val1 varint]...[valN varint][0 terminator]`
- Output record: `[sum varint]`
- Zig-zag encode: `(n << 1) ^ (n >> 63)`
- Zig-zag decode: `((val >> 1) as i64) ^ -((val & 1) as i64)`

No Avro, no schema registry. Raw bytes only.

---

## 4. Processor (Core Benchmark)

### Worker model

`std::thread` + per-thread `tokio::runtime::Runtime::new()` — same choice as `stream-test-rust` for fairest comparison.

### Phase 1 — Load (`BENCHMARK=1`)

- Single thread/runtime
- Produce 50,000,000 records to `integer-list-input-benchmark`
- Each record: random count (1–10 values), Zig-Zag LEB128 encoded, reuse a single `Vec<u8>` buffer
- Flush producer at end before spawning workers

### Phase 2 — Process

- `NUM_WORKERS` OS threads (default 8), each with its own `tokio::Runtime`
- Each thread owns its own krafka `Consumer` + `Producer`
- Consumer: `group_id = "stream-test-krafka"`, `auto_offset_reset = earliest`, `enable_auto_commit = true`
- Topics: input `integer-list-input-benchmark`, output `integer-sum-output-benchmark` (8 partitions each — must be pre-created)
- Per-record: decode Zig-Zag LEB128 input → compute sum → encode sum → produce to output topic
- Local batch counter: increment shared `AtomicU64 PROCESSED_COUNTER` every 1000 messages (reduces atomic contention)
- `AtomicU64 START_NANOS`: CAS on first message to record wall-clock start time
- Separate reporter thread: prints progress to stderr every 1s
- Exit condition: `PROCESSED_COUNTER >= 50_000_000`

### Output format

```
🏁 BENCHMARK RESULT (KRAFKA) 🏁
Total Records:   50000000
Processing Time: X.XXs
Throughput:      X,XXX,XXX msg/sec
```

### Non-benchmark mode (no `BENCHMARK` env var)

Runs Phase 2 only against `integer-list-input` / `integer-sum-output` (continuous, no exit condition).

---

## 5. Producer Binary (`producer.rs`)

- Infinite loop producing to `integer-list-input`
- Same Zig-Zag LEB128 encoding as processor Phase 1
- Log every 10,000 records
- SIGINT: flush producer, then exit

---

## 6. Consumer Binary (`consumer.rs`)

- Subscribe to `integer-sum-output`
- Decode Zig-Zag LEB128 sum from each record
- Print `Sum: {value}` per record
- SIGINT: exit

---

## 7. krafka API Usage

```rust
// Producer
Producer::builder()
    .bootstrap_servers(bootstrap)
    .build().await

producer.send(topic, None, &bytes).await
producer.flush().await

// Consumer
Consumer::builder()
    .bootstrap_servers(bootstrap)
    .group_id(group_id)
    .auto_offset_reset(AutoOffsetReset::Earliest)
    .enable_auto_commit(true)
    .build().await

consumer.subscribe(&[topic]).await
consumer.poll(Duration::from_millis(100)).await  // -> Result<Vec<ConsumerRecord>>
// record.value: Option<Bytes>
```

**No `broker.address.family` config** — use `127.0.0.1` directly for IPv4. Topics must exist before use (no auto-create).

---

## 8. Topics

| Topic | Partitions | Used by |
|---|---|---|
| `integer-list-input-benchmark` | 8 | benchmark load + process |
| `integer-sum-output-benchmark` | 8 | benchmark process output |
| `integer-list-input` | — | producer / run-stream |
| `integer-sum-output` | — | consumer / run-stream |

Benchmark topics must be pre-created by `make run-benchmark`. Non-benchmark topics must be pre-created separately if used.

---

## 9. Makefile

```makefile
BOOTSTRAP ?= 127.0.0.1:9094

build:
	cargo build --release

run-benchmark:
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-list-input-benchmark --partitions 8 --replication-factor 1
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-sum-output-benchmark --partitions 8 --replication-factor 1
	BENCHMARK=1 NUM_WORKERS=8 cargo run --release --bin processor

run-stream:
	cargo run --release --bin processor

run-producer:
	cargo run --release --bin producer

run-consumer:
	cargo run --release --bin consumer

docker-build:
	cd .. && docker build -f stream-test-krafka/Dockerfile -t stream-test-krafka .

docker-run:
	docker run --rm --network kafka-net \
	  -e BOOTSTRAP=kafka-broker:9092 \
	  stream-test-krafka
```

---

## 10. Dockerfile

```dockerfile
FROM rust:1-alpine3.23 AS builder
WORKDIR /usr/src/app
RUN apk add --no-cache musl-dev
COPY stream-test-krafka .
RUN cargo build --release

FROM alpine:3.23
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/processor /usr/local/bin/processor
COPY --from=builder /usr/src/app/target/release/producer /usr/local/bin/producer
COPY --from=builder /usr/src/app/target/release/consumer /usr/local/bin/consumer
CMD ["processor"]
```

Multi-stage Alpine. Builder requires only `musl-dev` — no C++ toolchain. Build context is repo root (`cd ..`) so the Dockerfile path is `stream-test-krafka/Dockerfile`. Runtime stage has zero extra dependencies.

---

## 11. Kafka Broker

- Local: `127.0.0.1:9094` (default `BOOTSTRAP`)
- Docker: `kafka-broker:9092` (passed via `-e BOOTSTRAP=kafka-broker:9092`)
- Broker: `apache/kafka:latest` running via `./run-kafka.sh`
