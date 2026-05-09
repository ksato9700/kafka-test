# Benchmark Summary (50 Million Records, 8 Workers)

| Language / Library | Best Processing Time | Best Throughput | Notes |
| :--- | :--- | :--- | :--- |
| **Java (Kafka Streams)** | 7.92 seconds | 6,312,468 msg/sec | |
| **C (librdkafka)** | 10.59 seconds | 4,719,502 msg/sec | |
| **Rust (rdkafka)** | 14.29 seconds | 3,499,934 msg/sec | librdkafka bindings |
| **Go (confluent-kafka-go)** | 20.43 seconds | 2,447,551 msg/sec | |
| **Rust (krafka 0.7, Acks::None)** | 270.52 seconds | 184,827 msg/sec | Pure Rust; `Acks::None`, `linger=5ms`, `batch_size=64KB` — no broker ACK wait |
| **Rust (krafka 0.7, Acks::Leader)** | 307.47 seconds | 162,617 msg/sec | Pure Rust; `Acks::Leader`, `linger=5ms`, `batch_size=64KB`, `idempotent=false` |

## Key Observations
*   **Java** achieved the highest peak performance, reaching over 6.3 million messages per second.
*   **C** maintained consistent performance, averaging around 4.5 million messages per second.
*   **Rust (rdkafka)** peaked at approximately 3.5 million messages per second using librdkafka's fire-and-forget `ThreadedProducer`.
*   **Go** processed at approximately 2.3 million messages per second.
*   **Rust (krafka 0.7)** throughput depends heavily on producer configuration. Default settings (`Acks::All`, `linger=0`) yield only ~15–29K msg/sec. With `Acks::Leader` + batching: ~163K msg/sec. With `Acks::None` + batching: ~185K msg/sec. The remaining ~19× gap vs rdkafka is structural: krafka's worker still awaits socket writes through the Tokio executor even with `Acks::None`, whereas rdkafka's background C thread fully decouples the consume loop from all I/O.
*   All tests were performed on a backlog of 50,000,000 records.

## Platform

All results above were collected on:

- **Machine:** MacBook Pro (Apple M4)
- **CPU:** Apple M4, 10 cores
- **RAM:** 24 GB
- **OS:** macOS 26.4.1 (Darwin 25.4.0)
- **Kafka:** 4.2.0 (KRaft mode, single broker, 8 partitions, replication factor 1)
- **Rust:** 1.94.1
- **Java/Kafka Streams:** Amazon Corretto 21

> Note: Platform info was not recorded during the original Java/C/Rust/Go runs; the machine details above are from when the krafka run was performed on the same hardware.
