# Design Document: Ruby Streaming Sum Processor

## 1. Objective
To implement the `streaming-performance-spec.md` in Ruby. While Ruby is generally slower than Java or Rust for CPU-bound tasks, we aim for maximum possible throughput by using a C-backed Kafka client and manual binary SerDe.

## 2. Technology Stack
- **Language**: Ruby 3.3+
- **Kafka Library**: `rdkafka` gem (C-extension wrapping `librdkafka`).
- **Concurrency**: Multi-threading (utilizing `librdkafka`'s internal threading and Ruby's `Thread` for worker isolation).
- **Serialization**: Manual Zig-Zag binary encoding/decoding (matching the Java/Rust "extreme" optimizations).

## 3. Optimizations
- **Manual SerDe**: Bypassing the `avro` gem's generic mapping to avoid object allocation and schema validation overhead.
- **Pipelining**: Using `rdkafka`'s asynchronous producer.
- **Buffer Reuse**: Reusing string buffers for encoding where possible.
- **Throughput Reporting**: Atomic-like counter reporting every 5 seconds.

## 4. Concurrency Note
Due to Ruby's Global Interpreter Lock (GIL), true CPU parallelism for the "Sum" calculation is limited within a single process. However, `rdkafka` performs I/O and compression in background C threads, allowing us to still achieve significant throughput.
