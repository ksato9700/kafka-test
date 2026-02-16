# Specification: High-Performance Streaming Sum Test

## 1. Objective
To provide a standardized benchmark for comparing Kafka streaming performance across different languages and frameworks. The core task is to consume a list of integers, sum them, and produce the result back to Kafka.

## 2. System Architecture
- **Source Topic**: `integer-list-input-avro`
- **Sink Topic**: `integer-sum-output-avro`
- **Pattern**: Pipe-and-Filter (Consume -> Transform -> Produce)
- **State**: Stateless (each message is processed independently)

## 3. Data Specification (Avro)

Implementation must use binary Avro serialization without a Schema Registry for maximum throughput in testing.

### Input: `IntegerList`
```json
{
  "type": "record",
  "name": "IntegerList",
  "namespace": "com.example.kafka",
  "fields": [
    {"name": "numbers", "type": {"type": "array", "items": "int"}}
  ]
}
```

### Output: `SumResult`
```json
{
  "type": "record",
  "name": "SumResult",
  "namespace": "com.example.kafka",
  "fields": [
    {"name": "sum", "type": "long"}
  ]
}
```

## 4. Program Logic
For every message received:
1. Decode the binary Avro payload into a list of integers.
2. Calculate the sum of the integers.
3. Encode the sum into a `SumResult` binary Avro payload.
4. Send the result to the sink topic.

## 5. Performance Requirements & Optimizations

To ensure valid performance comparisons, implementations should adhere to these guidelines:

### Monitoring
- The program must track the number of messages processed.
- Every 5 seconds, it must log the average throughput: `(current_count - previous_count) / 5`.

### Optimizations
- **Object Reuse**: Avoid allocating new encoders, decoders, or buffers per message. Use thread-local storage or object pooling.
- **Batching**: Use a producer batch size of at least 64KB and a linger time of 10-20ms.
- **Compression**: Use `snappy` compression for production.
- **Multi-threading**: The implementation should support configurable parallelism (e.g., via environment variables) to match the number of Kafka topic partitions.

## 6. Implementation Checklist
- [ ] Implement Avro serialization/deserialization logic.
- [ ] Connect to Kafka using configurable bootstrap servers.
- [ ] Implement the sum transformation.
- [ ] Add the 5-second throughput reporting logic.
- [ ] Ensure the producer is tuned for high throughput (batching/compression).
- [ ] Provide a `Makefile` and `Dockerfile` for consistent deployment.

## 7. Standard Topic Configuration
For benchmarking, topics should be pre-created with:
- **Partitions**: 3 (minimum)
- **Replication Factor**: 1 (for local testing)
