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
