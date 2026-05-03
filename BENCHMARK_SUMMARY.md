# Benchmark Summary (50 Million Records, 8 Workers)

| Language / Library | Best Processing Time | Best Throughput | Notes |
| :--- | :--- | :--- | :--- |
| **Java (Kafka Streams)** | 7.92 seconds | 6,312,468 msg/sec | |
| **C (librdkafka)** | 10.59 seconds | 4,719,502 msg/sec | |
| **Rust (rdkafka)** | 14.29 seconds | 3,499,934 msg/sec | librdkafka bindings |
| **Go (confluent-kafka-go)** | 20.43 seconds | 2,447,551 msg/sec | |
| **Rust (krafka 0.7)** | 1741.55 seconds | 28,710 msg/sec | Pure Rust; `join_all` over each poll batch — concurrent within batch, but poll batches are small so inter-batch serialization remains the bottleneck |

## Key Observations
*   **Java** achieved the highest peak performance, reaching over 6.3 million messages per second.
*   **C** maintained consistent performance, averaging around 4.5 million messages per second.
*   **Rust (rdkafka)** peaked at approximately 3.5 million messages per second using librdkafka's fire-and-forget `ThreadedProducer`.
*   **Go** processed at approximately 2.3 million messages per second.
*   **Rust (krafka 0.7)** achieved ~29K msg/sec with `join_all` over each poll batch (up from ~15K with sequential `.await`). The fundamental bottleneck is that krafka has no fire-and-forget producer path: every send future must be driven to completion before the result is known. `join_all` concurrently drives sends within a single poll batch, but krafka's poll batches are small, so the next poll can't start until the current batch's sends all ACK. rdkafka's `ThreadedProducer` avoids this entirely by batching and delivering asynchronously in a background thread.
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
