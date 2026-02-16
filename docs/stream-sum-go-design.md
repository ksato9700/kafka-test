# Design Document: Go Streaming Sum Processor

## 1. Objective
To implement the `streaming-performance-spec.md` in Go. We aim to achieve performance close to Rust/Java by utilizing Go's efficient concurrency (goroutines) and fast binary processing.

## 2. Technology Stack
- **Language**: Go 1.22+
- **Kafka Library**: `confluent-kafka-go` (v2), which wraps the C-based `librdkafka` for maximum throughput.
- **Concurrency**: Goroutines (one per Kafka partition/worker).
- **Serialization**: Manual Zig-Zag binary encoding/decoding.

## 3. Optimizations
- **Direct Binary SerDe**: Implementing the Zig-Zag logic using the `encoding/binary` style but optimized for zero-allocation.
- **Worker Pool**: Spawning independent goroutines, each managing its own consumer/producer cycle to avoid channel overhead in the hot path.
- **CGO Tuning**: Minimizing CGO transitions by batching messages where possible.

## 4. Architecture
Unlike Python/Ruby which require multiple processes, Go can efficiently manage all workers within a single process. Each worker goroutine will:
1. Fetch a message from the consumer.
2. Manually decode the Avro bytes.
3. Calculate the sum.
4. Manually encode the result.
5. Send to the producer.
