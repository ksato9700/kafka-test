# Design Document: Kafka Streams Sum Processor

## Objective
The goal of this project is to implement a Kafka Streams application in Java that processes a continuous stream of integer lists, calculates the sum of each list, and sends the result to an output topic. This will serve as a baseline for testing streaming capability and performance.

## Architecture
The application uses the Kafka Streams library to define a processing topology.

### Data Flow
1. **Source**: Reads from an input topic (default: `integer-list-input`).
2. **Processor**:
    - Deserializes the input message (JSON list of integers).
    - Sums all integers in the list.
    - Serializes the result (JSON object with the sum).
3. **Sink**: Writes the result to an output topic (default: `integer-sum-output`).

### Technology Stack
- **Language**: Java 21
- **Framework**: Kafka Streams 3.7.0
- **Serialization**: Avro
- **Build Tool**: Gradle 8.x

## Data Formats

### Input Topic: `integer-list-input-avro`
Avro Record: `IntegerList`
- `numbers`: array of integers

### Output Topic: `integer-sum-output-avro`
Avro Record: `SumResult`
- `sum`: long

## Configuration
The following environment variables can be used to configure the application:
- `KAFKA_BOOTSTRAP_SERVERS`: Kafka broker addresses (default: `localhost:9094`).
- `INPUT_TOPIC`: Topic to read from (default: `integer-list-input`).
- `OUTPUT_TOPIC`: Topic to write to (default: `integer-sum-output`).
- `APPLICATION_ID`: Kafka Streams application ID (default: `stream-sum-test-java`).
- `STREAMS_*`: Any environment variable starting with `STREAMS_` will be converted to a Kafka Streams configuration property. For example, `STREAMS_NUM_STREAM_THREADS=4` becomes `num.stream.threads=4`.

## Performance Considerations
- **Parallelism**: The application can be scaled by increasing the number of partitions in the input topic and running multiple instances of the application with the same `APPLICATION_ID`.
- **Latency**: Minimal processing logic ensures low latency per record.
- **Throughput**: Kafka Streams' efficient threading model allows for high throughput.

## Project Structure
`stream-test-java/`
├── build.gradle          # Build configuration
├── settings.gradle       # Gradle settings
├── Dockerfile            # Containerization for deployment
├── Makefile              # Convenience commands
└── src/main/java/com/example/kafka/
    ├── StreamSumApp.java # Main application entry point and topology
    └── model/
        ├── IntegerList.java # POJO for input
        └── SumResult.java   # POJO for output
