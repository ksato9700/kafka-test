# Design Document: C Streaming Sum Processor

## 1. Objective
To implement the `streaming-performance-spec.md` in C using `librdkafka`. This implementation aims to reach the theoretical maximum performance by bypassing all high-level language runtimes, garbage collectors, and FFI overhead.

## 2. Technology Stack
- **Language**: C (ISO C11)
- **Kafka Library**: `librdkafka` (native C library)
- **Concurrency**: POSIX Threads (pthreads) with per-worker Kafka handles.
- **Serialization**: Manual Zig-Zag binary encoding/decoding.

## 3. Optimizations
- **Direct Memory Access**: Direct manipulation of byte buffers without intermediate object representations.
- **Zero Runtime Overhead**: No GC, no JIT, no interpreter overhead.
- **High-Speed Polling**: Using `rd_kafka_consume_batch` or optimized `rd_kafka_poll` loops.
- **Contention-Free workers**: Each thread has its own `rd_kafka_t` consumer and producer instance.

## 4. Architecture
The program will implement the "Benchmark Mode":
1. **Phase 1 (Load)**: A high-speed C producer loop filling the topic with 50,000,000 records.
2. **Phase 2 (Process)**: Spawning N worker threads. Each thread pulls a batch of messages, performs the Zig-Zag decode, calculates the sum, Zig-Zag encodes, and produces the result.
3. **Phase 3 (Report)**: Atomic counter tracks progress and reports final timing.
