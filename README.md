# Kafka Multi-Language Test Project

This repository contains interoperable Kafka Producer and Consumer implementations in multiple programming languages (**Rust**, **Python**, **Java**, **Scala**, **Ruby**). It is designed to demonstrate consistent behavior, logging, and error handling across different tech stacks.

## 🚀 Quick Start (Docker)

The easiest way to run the project is using Docker. This avoids setting up local language environments.

### 1. Start the Kafka Broker
This script sets up a single-node Kafka broker compatible with both Docker and local clients.

```bash
./run-kafka.sh
```

### 2. Run a Consumer
In a new terminal:

```bash
# Example: Run the Rust consumer
cd kafka-test-rust
make docker-run-consumer
```

### 3. Run a Producer
In another terminal:

```bash
# Example: Run the Python producer
cd kafka-test-python
make docker-run-producer
```

The consumer should immediately start receiving messages sent by the producer. You can mix and match languages (e.g., Python Producer + Java Consumer).

## 📂 Project Structure

- **`run-kafka.sh`**: Helper script to start the Kafka broker in Docker with correct networking configuration.
- **`kafka-test-rust/`**: Rust implementation (`rdkafka`).
- **`kafka-test-python/`**: Python implementation (`kafka-python`).
- **`kafka-test-java/`**: Java implementation (`kafka-clients` via Gradle).
- **`kafka-test-scala/`**: Scala implementation (`kafka-clients` via SBT).
- **`kafka-test-ruby/`**: Ruby implementation (`rdkafka-ruby`).
- **`docs/`**: Design documents and specifications.

## 🛠️ Local Development

To run clients natively on your host machine (Mac/Linux), ensure you have the respective language toolchains installed (`cargo`, `python`, `jdk`, `sbt`, `ruby`).

### 1. Networking
The broker exposes port **9094** for local clients (`127.0.0.1:9094`).
The Docker clients use port **9092** (`kafka-broker:9092`).

All implementations are configured to auto-detect the environment or default to the correct local settings.

### 2. Running Locally

**Rust:**
```bash
cd kafka-test-rust
make run-consumer
make run-producer
```

**Python:**
```bash
cd kafka-test-python
make run-consumer
make run-producer
```

**Java (requires JDK 21+):**
```bash
cd kafka-test-java
make run-consumer
make run-producer
```

**Scala (requires JDK 21+):**
```bash
cd kafka-test-scala
make run-consumer
make run-producer
```

**Ruby:**
```bash
cd kafka-test-ruby
make install  # Install gems first
make run-consumer
make run-producer
```

## 📝 Features

All implementations follow a strict design spec (see `docs/kafka-client-design.md`) including:
- **Structured Logging:** Timestamps, log levels (INFO/WARN/ERROR).
- **Latency Calculation:** Measures end-to-end latency in seconds.
- **Graceful Shutdown:** Handles `Ctrl-C` (SIGINT) to ensure offsets are committed.
- **IPv4 Enforcement:** Fixes common `localhost` IPv6 resolution issues on macOS.
- **Dockerized:** Minimal Alpine-based images.

## ⚠️ Troubleshooting

- **"Connection Refused"**: Ensure `./run-kafka.sh` is running. If on Docker, ensure you are using `make docker-run-*` targets which utilize the `kafka-net` network.
- **Consumer receives nothing**:
    - The default offset reset is `latest`. If the producer finished sending messages *before* you started the consumer, you won't see them.
    - Start the consumer *first*, or set `AUTO_OFFSET_RESET=earliest`.
    - Check if the consumer group is stuck rebalancing (wait ~10s).
- **Broker logs "Sent auto-creation request for __consumer_offsets..."**:
    - This happens if you run a standard Kafka image without configuring replication factor.
    - **Fix:** Use `./run-kafka.sh` which sets `KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1`.
