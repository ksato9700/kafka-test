# Design Document: Scala Streaming Sum Processor

## 1. Objective
To implement the `streaming-performance-spec.md` in Scala. To achieve performance parity with the Java implementation, we will use the low-level Kafka Java Client directly and implement manual Zig-Zag binary SerDe.

## 2. Technology Stack
- **Language**: Scala 3.3+
- **Build Tool**: sbt
- **Kafka Library**: `kafka-clients` (Java native client)
- **Concurrency**: Java Threads with per-worker Consumer/Producer instances.
- **Serialization**: Manual Zig-Zag binary encoding/decoding.

## 3. Optimizations
- **Direct Byte Manipulation**: Using `java.io.ByteArrayInputStream` and `ByteArrayOutputStream` to match the Java "Ultra-Performance" logic.
- **Avoid Scala Collection Overhead**: Using primitive loops and avoiding intermediate object allocations (like `List` or `Option`) in the hot path.
- **Thread Isolation**: Each worker thread manages its own lifecycle to eliminate lock contention.
- **Batching**: Global throughput metrics are updated in batches.

## 4. Architecture
The Scala implementation will closely mirror the optimized Java version, as they share the same underlying JVM. We will focus on writing "performance-idiomatic" Scala that avoids unnecessary allocations while maintaining the language's expressiveness for setup and configuration.
