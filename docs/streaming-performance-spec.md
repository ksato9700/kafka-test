# Specification: High-Performance Streaming Sum Test

## 1. Objective
To provide a standardized benchmark for comparing Kafka streaming performance across different languages and frameworks. The core task is to consume a list of integers, sum them, and produce the result back to Kafka.

## 2. System Architecture
- **Source Topic**: `integer-list-input-avro`
- **Sink Topic**: `integer-sum-output-avro`
- **Pattern**: Pipe-and-Filter (Consume -> Transform -> Produce)
- **State**: Stateless (each message is processed independently)

## 3. Data Specification (Zig-Zag Binary)

For "Extreme Performance" benchmarks, implementations should use manual **Zig-Zag binary encoding** (LEB128) instead of generic Avro libraries. This eliminates reflection and object allocation overhead.

### Input: `IntegerList`
- **Format**: `[Count (Zig-Zag)][Int1 (Zig-Zag)][Int2 (Zig-Zag)]...[0]`
- The array ends with a `0` (or the count specifies the length).

### Output: `SumResult`
- **Format**: `[Sum (Zig-Zag Long)]`

## 4. Program Logic
For every message received:
1. Decode the binary payload (Zig-Zag).
2. Calculate the sum of the integers.
3. Encode the sum into a Zig-Zag binary payload.
4. Send the result to the sink topic.

## 5. Performance Requirements & Optimizations

To ensure valid performance comparisons, implementations should adhere to these guidelines:

### Monitoring
- The program must track the number of messages processed.
- Every second (or 5s), it should log progress if in Benchmark mode.

### Optimizations
- **Zero-Allocation**: Reuse buffers and avoid per-message allocations.
- **Batching**: Use a producer batch size of at least 100,000 messages and a linger time of 20ms.
- **Compression**: Use `snappy` compression.
- **Multi-threading**: The implementation should support configurable parallelism via `NUM_WORKERS`.

## 6. Batch Benchmark Methodology
To ensure a fair comparison without resource contention from the producer, implementations should support a "Batch Benchmark" mode:
1. **Load Phase**: Produce 50,000,000 messages to the input topic as fast as possible.
2. **Flush**: Ensure all messages are fully acknowledged by the broker.
3. **Process Phase**: 
    - Start a timer.
    - Consume and process exactly 50,000,000 messages.
    - Produce results to the output topic.
    - Stop the timer when the 50,000,000th result is produced.
4. **Result**: Calculate `50,000,000 / total_seconds` for the final score.

## 7. Implementation Checklist
- [x] Implement manual Zig-Zag serialization/deserialization logic.
- [x] Connect to Kafka using configurable bootstrap servers.
- [x] Implement the sum transformation.
- [x] Add throughput reporting logic.
- [x] Ensure the producer is tuned for high throughput (batching/compression).
- [x] Provide a `Makefile` and `Dockerfile` for consistent deployment.

## 7. Standard Topic Configuration
For benchmarking, topics should be pre-created with:
- **Partitions**: 3 (minimum)
- **Replication Factor**: 1 (for local testing)
