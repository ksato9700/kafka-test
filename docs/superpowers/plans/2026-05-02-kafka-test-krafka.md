# kafka-test-krafka Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `kafka-test-krafka/` — a Kafka Producer and Consumer using the pure-Rust `krafka` crate — with full feature parity to `kafka-test-rust` (rdkafka) and a simpler build pipeline.

**Architecture:** Two binaries (`kafka-producer`, `kafka-consumer`) in a single Cargo crate. Producer sends raw Avro-encoded messages every 2 seconds; Consumer polls and decodes them, logging latency. No C dependencies — krafka is pure Rust so the Dockerfile builder stage needs only `musl-dev`.

**Tech Stack:** Rust 2024 edition, krafka 0.7, apache-avro 0.21, tokio 1 (full), tracing + tracing-subscriber 0.3.

---

## File Map

| File | Purpose |
|---|---|
| `kafka-test-krafka/Cargo.toml` | Crate manifest, two `[[bin]]` entries |
| `kafka-test-krafka/src/producer.rs` | Producer binary |
| `kafka-test-krafka/src/consumer.rs` | Consumer binary |
| `kafka-test-krafka/Makefile` | Standard run/docker targets |
| `kafka-test-krafka/Dockerfile` | Multi-stage Alpine build |
| `kafka-test-krafka/.dockerignore` | Exclude target/ from Docker context |
| `kafka-test-krafka/.gitignore` | Exclude target/ from git |
| `kafka-test-krafka/README.md` | Usage and comparison notes |

---

## Task 1: Scaffold Cargo crate

**Files:**
- Create: `kafka-test-krafka/Cargo.toml`
- Create: `kafka-test-krafka/src/producer.rs` (stub)
- Create: `kafka-test-krafka/src/consumer.rs` (stub)

- [ ] **Step 1: Create directory structure**

```bash
mkdir -p kafka-test-krafka/src
```

- [ ] **Step 2: Create `kafka-test-krafka/Cargo.toml`**

```toml
[package]
name = "kafka-test-krafka"
version = "0.1.0"
edition = "2024"

[dependencies]
apache-avro = "0.21"
krafka = "0.7"
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"

[[bin]]
name = "kafka-producer"
path = "src/producer.rs"

[[bin]]
name = "kafka-consumer"
path = "src/consumer.rs"
```

- [ ] **Step 3: Create stub `kafka-test-krafka/src/producer.rs`**

```rust
fn main() {}
```

- [ ] **Step 4: Create stub `kafka-test-krafka/src/consumer.rs`**

```rust
fn main() {}
```

- [ ] **Step 5: Verify the crate compiles**

```bash
cd kafka-test-krafka && cargo build 2>&1 | tail -5
```

Expected: `Finished` with no errors. `krafka 0.7.x` will appear in the dependency list.

- [ ] **Step 6: Commit**

```bash
git add kafka-test-krafka/
git commit -m "feat(krafka): scaffold Cargo crate with stub binaries"
```

---

## Task 2: Implement Producer

**Files:**
- Modify: `kafka-test-krafka/src/producer.rs`

- [ ] **Step 1: Write `kafka-test-krafka/src/producer.rs`**

```rust
use apache_avro::Schema;
use apache_avro::types::Record;
use krafka::producer::Producer;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;

fn load_schema() -> Schema {
    let paths = [
        "../schemas/message.avsc",
        "/app/schemas/message.avsc",
        "schemas/message.avsc",
    ];
    for p in paths {
        if Path::new(p).exists() {
            let content = fs::read_to_string(p).expect("Failed to read schema file");
            return Schema::parse_str(&content).expect("Failed to parse schema");
        }
    }
    panic!("Schema file not found in paths: {:?}", paths);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Hello from kafka-test-krafka!");

    let schema = load_schema();
    tracing::info!("Loaded Avro schema");

    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());

    let producer = Producer::builder()
        .bootstrap_servers(&bootstrap_servers)
        .client_id("kafka-test-krafka-producer")
        .build()
        .await
        .expect("Failed to create producer");

    tracing::info!("🚀 Producer is now running...");

    let mut message_id: i64 = 0;
    loop {
        let event_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let mut record = Record::new(&schema).unwrap();
        record.put("message_id", message_id);
        record.put("event_time", event_time);
        record.put("content", format!("Message {}", message_id));

        let payload =
            apache_avro::to_avro_datum(&schema, record).expect("Failed to serialize to Avro");
        let n = payload.len();

        match producer.send(&topic, None, &payload).await {
            Ok(_) => tracing::info!("🚀 Sent: Message {} ({} bytes)", message_id, n),
            Err(e) => tracing::error!("❌ Error sending message: {:?}", e),
        }

        message_id += 1;

        tokio::select! {
            _ = time::sleep(Duration::from_secs(2)) => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("🛑 Shutting down producer...");
                producer.flush().await.expect("Failed to flush producer");
                break;
            }
        }
    }
}
```

- [ ] **Step 2: Verify producer compiles**

```bash
cd kafka-test-krafka && cargo build --bin kafka-producer 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add kafka-test-krafka/src/producer.rs
git commit -m "feat(krafka): implement producer with Avro serialization and graceful shutdown"
```

---

## Task 3: Implement Consumer

**Files:**
- Modify: `kafka-test-krafka/src/consumer.rs`

- [ ] **Step 1: Write `kafka-test-krafka/src/consumer.rs`**

```rust
use apache_avro::Schema;
use apache_avro::types::Value;
use krafka::consumer::{AutoOffsetReset, Consumer};
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn load_schema() -> Schema {
    let paths = [
        "../schemas/message.avsc",
        "/app/schemas/message.avsc",
        "schemas/message.avsc",
    ];
    for p in paths {
        if Path::new(p).exists() {
            let content = fs::read_to_string(p).expect("Failed to read schema file");
            return Schema::parse_str(&content).expect("Failed to parse schema");
        }
    }
    panic!("Schema file not found in paths: {:?}", paths);
}

fn parse_offset_reset(s: &str) -> AutoOffsetReset {
    match s {
        "earliest" => AutoOffsetReset::Earliest,
        "none" => AutoOffsetReset::None,
        _ => AutoOffsetReset::Latest,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let schema = load_schema();
    tracing::info!("Loaded Avro schema");

    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());
    let group_id = env::var("CONSUMER_GROUP_ID").unwrap_or_else(|_| "my-group-1".to_string());
    let auto_offset_reset_str =
        env::var("AUTO_OFFSET_RESET").unwrap_or_else(|_| "latest".to_string());
    let auto_offset_reset = parse_offset_reset(&auto_offset_reset_str);

    tracing::info!(
        "🛠️ Connecting KafkaConsumer to topic '{}' at '{}' (group: '{}')...",
        topic,
        bootstrap_servers,
        group_id
    );

    let consumer = Consumer::builder()
        .bootstrap_servers(&bootstrap_servers)
        .group_id(&group_id)
        .auto_offset_reset(auto_offset_reset)
        .enable_auto_commit(true)
        .build()
        .await
        .expect("Failed to create consumer");

    consumer
        .subscribe(&[topic.as_str()])
        .await
        .expect("Failed to subscribe to topic");

    tracing::info!("📥 Listening for messages...");

    loop {
        tokio::select! {
            result = consumer.poll(Duration::from_millis(100)) => {
                match result {
                    Ok(records) => {
                        for record in records {
                            if let Some(ref value_bytes) = record.value {
                                let mut reader = value_bytes.as_ref();
                                match apache_avro::from_avro_datum(&schema, &mut reader, Some(&schema)) {
                                    Ok(Value::Record(fields)) => {
                                        let message_id = fields
                                            .iter()
                                            .find(|(k, _)| k == "message_id")
                                            .and_then(|(_, v)| match v {
                                                Value::Long(l) => Some(*l),
                                                Value::Int(i) => Some(*i as i64),
                                                _ => None,
                                            })
                                            .unwrap_or(0);

                                        let event_time = fields
                                            .iter()
                                            .find(|(k, _)| k == "event_time")
                                            .and_then(|(_, v)| match v {
                                                Value::Double(d) => Some(*d),
                                                Value::Float(f) => Some(*f as f64),
                                                _ => None,
                                            })
                                            .unwrap_or(0.0);

                                        let content = fields
                                            .iter()
                                            .find(|(k, _)| k == "content")
                                            .and_then(|(_, v)| match v {
                                                Value::String(s) => Some(s.clone()),
                                                _ => None,
                                            })
                                            .unwrap_or_else(|| "Unknown".to_string());

                                        let received_time = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .expect("Time went backwards")
                                            .as_secs_f64();
                                        let latency = received_time - event_time;

                                        tracing::info!(
                                            "📨 New message: [ID={}] {} at {:.3}",
                                            message_id,
                                            content,
                                            event_time
                                        );
                                        tracing::info!("⏱️ Latency: {:.3} seconds\n", latency);
                                    }
                                    Ok(other) => tracing::warn!("⚠️ Unexpected Avro value type: {:?}", other),
                                    Err(e) => tracing::warn!("⚠️ Avro deserialization error: {:?}", e),
                                }
                            }
                        }
                    }
                    Err(e) => tracing::error!("❌ Poll error: {:?}", e),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("🛑 Shutting down gracefully...");
                consumer.commit().await.expect("Failed to commit offsets");
                break;
            }
        }
    }
}
```

- [ ] **Step 2: Verify consumer compiles**

```bash
cd kafka-test-krafka && cargo build --bin kafka-consumer 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add kafka-test-krafka/src/consumer.rs
git commit -m "feat(krafka): implement consumer with Avro deserialization, latency logging, and graceful shutdown"
```

---

## Task 4: Makefile, Dockerfile, and support files

**Files:**
- Create: `kafka-test-krafka/Makefile`
- Create: `kafka-test-krafka/Dockerfile`
- Create: `kafka-test-krafka/.dockerignore`
- Create: `kafka-test-krafka/.gitignore`

- [ ] **Step 1: Create `kafka-test-krafka/Makefile`**

```makefile
.PHONY: run-consumer run-producer docker-build docker-run-consumer docker-run-producer

run-consumer:
	cargo run --bin kafka-consumer

run-producer:
	cargo run --bin kafka-producer

docker-build:
	cd .. && docker build -f kafka-test-krafka/Dockerfile -t kafka-test-krafka .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-krafka kafka-consumer

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-krafka kafka-producer
```

Note: indentation in Makefile must use **tabs**, not spaces.

- [ ] **Step 2: Create `kafka-test-krafka/Dockerfile`**

```dockerfile
FROM rust:1-alpine3.23 AS builder

WORKDIR /usr/src/app

RUN apk add --no-cache musl-dev

COPY kafka-test-krafka .

RUN cargo build --release

FROM alpine:3.23

WORKDIR /app

COPY schemas /app/schemas
COPY --from=builder /usr/src/app/target/release/kafka-consumer /usr/local/bin/kafka-consumer
COPY --from=builder /usr/src/app/target/release/kafka-producer /usr/local/bin/kafka-producer

CMD ["kafka-consumer"]
```

- [ ] **Step 3: Create `kafka-test-krafka/.dockerignore`**

```
target/
```

- [ ] **Step 4: Create `kafka-test-krafka/.gitignore`**

```
/target
```

- [ ] **Step 5: Commit**

```bash
git add kafka-test-krafka/Makefile kafka-test-krafka/Dockerfile kafka-test-krafka/.dockerignore kafka-test-krafka/.gitignore
git commit -m "feat(krafka): add Makefile, Dockerfile, and ignore files"
```

---

## Task 5: README

**Files:**
- Create: `kafka-test-krafka/README.md`

- [ ] **Step 1: Create `kafka-test-krafka/README.md`**

```markdown
# kafka-test-krafka

Kafka Producer and Consumer implemented in Rust using [krafka](https://crates.io/crates/krafka) — a pure-Rust, async-native Kafka client with no C dependencies.

This is a companion to `kafka-test-rust` (which uses `rdkafka`) for direct library comparison.

## Requirements

- Kafka broker 3.9+ (krafka minimum)
- Rust toolchain (for local runs)

## Local Usage

Start the broker first:
```bash
cd .. && ./run-kafka.sh
```

Run consumer (new terminal):
```bash
make run-consumer
```

Run producer (another terminal):
```bash
make run-producer
```

## Docker Usage

```bash
make docker-run-consumer   # terminal 1
make docker-run-producer   # terminal 2
```

## Configuration

| Variable | Default (local) | Docker |
|---|---|---|
| `KAFKA_BOOTSTRAP_SERVERS` | `127.0.0.1:9094` | `kafka-broker:9092` |
| `TOPIC_NAME` | `my-topic-1` | `my-topic-1` |
| `CONSUMER_GROUP_ID` | `my-group-1` | `my-group-1` |
| `AUTO_OFFSET_RESET` | `latest` | `latest` |

## Comparison: krafka vs rdkafka

| Concern | kafka-test-rust (rdkafka) | kafka-test-krafka (krafka) |
|---|---|---|
| C dependencies | Yes (librdkafka via cmake) | None |
| Builder stage apk packages | ~10 packages | `musl-dev` only |
| Runtime deps | libgcc, libstdc++ | None |
| IPv4 forcing | `broker.address.family=v4` | Use `127.0.0.1` directly |
| Min Kafka version | Any | 3.9+ |
| Consumer API style | Stream | Poll loop |
```

- [ ] **Step 2: Commit**

```bash
git add kafka-test-krafka/README.md
git commit -m "docs(krafka): add README with usage and rdkafka comparison table"
```

---

## Task 6: Local smoke test

Prerequisites: Kafka broker running (`./run-kafka.sh` from repo root).

- [ ] **Step 1: Start consumer in one terminal**

```bash
cd kafka-test-krafka && make run-consumer
```

Expected output (within 10s):
```
INFO kafka_test_krafka: Loaded Avro schema
INFO kafka_test_krafka: 🛠️ Connecting KafkaConsumer to topic 'my-topic-1' at '127.0.0.1:9094' (group: 'my-group-1')...
INFO kafka_test_krafka: 📥 Listening for messages...
```

- [ ] **Step 2: Start producer in another terminal**

```bash
cd kafka-test-krafka && make run-producer
```

Expected output:
```
INFO kafka_test_krafka: Hello from kafka-test-krafka!
INFO kafka_test_krafka: Loaded Avro schema
INFO kafka_test_krafka: 🚀 Producer is now running...
INFO kafka_test_krafka: 🚀 Sent: Message 0 (... bytes)
```

- [ ] **Step 3: Verify consumer receives messages**

Consumer terminal should show:
```
INFO kafka_test_krafka: 📨 New message: [ID=0] Message 0 at ...
INFO kafka_test_krafka: ⏱️ Latency: 0.00... seconds
```

- [ ] **Step 4: Verify cross-language interop**

Stop the krafka producer. Start the rdkafka producer in `kafka-test-rust` while keeping the krafka consumer running:

```bash
cd ../kafka-test-rust && make run-producer
```

The krafka consumer should continue receiving and decoding messages — confirming Avro parity.

- [ ] **Step 5: Verify graceful shutdown**

Send `Ctrl-C` to the consumer. Expected:
```
INFO kafka_test_krafka: 🛑 Shutting down gracefully...
```
Process exits cleanly (no panic, no hanging).

---

## Task 7: Docker smoke test

Prerequisites: Kafka broker running (`./run-kafka.sh`).

- [ ] **Step 1: Build Docker image**

```bash
cd kafka-test-krafka && make docker-build 2>&1 | tail -5
```

Expected: `Successfully tagged kafka-test-krafka:latest` (or `naming to docker.io/library/kafka-test-krafka:latest`). Builder stage should show only `musl-dev` being installed — no cmake/g++.

- [ ] **Step 2: Run Docker consumer**

```bash
make docker-run-consumer
```

Expected: same startup logs as local, now connecting to `kafka-broker:9092`.

- [ ] **Step 3: Run Docker producer (new terminal)**

```bash
cd kafka-test-krafka && make docker-run-producer
```

Expected: producer sends messages; consumer terminal shows received messages with latency.

- [ ] **Step 4: Commit final state**

```bash
git add -A
git commit -m "feat(krafka): kafka-test-krafka complete — producer, consumer, Docker verified"
```
