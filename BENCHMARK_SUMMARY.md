# Benchmark Summary (50 Million Records, 8 Workers)

| Language / Library | Best Processing Time | Best Throughput | Notes |
| :--- | :--- | :--- | :--- |
| **Java (Kafka Streams)** | 7.92 seconds | 6,312,468 msg/sec | |
| **C (librdkafka)** | 10.59 seconds | 4,719,502 msg/sec | |
| **Rust (rdkafka)** | 14.29 seconds | 3,499,934 msg/sec | librdkafka bindings |
| **Go (confluent-kafka-go)** | 20.43 seconds | 2,447,551 msg/sec | |
| **Rust (krafka 0.7)** | 3311.04 seconds | 15,101 msg/sec | Pure Rust; `producer.send().await` awaits ACK per message — no pipelining in Phase 2 |

## Key Observations
*   **Java** achieved the highest peak performance, reaching over 6.3 million messages per second.
*   **C** maintained consistent performance, averaging around 4.5 million messages per second.
*   **Rust (rdkafka)** peaked at approximately 3.5 million messages per second using librdkafka's fire-and-forget `ThreadedProducer`.
*   **Go** processed at approximately 2.3 million messages per second.
*   **Rust (krafka 0.7)** achieved only ~15K msg/sec. The bottleneck is that krafka's `producer.send().await` awaits broker acknowledgement for each message individually — there is no built-in batch/fire-and-forget API equivalent to rdkafka's `ThreadedProducer`. With pipelining (async task per send), Phase 1 loading completed normally, but Phase 2's processing loop still awaits each output send sequentially.
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
