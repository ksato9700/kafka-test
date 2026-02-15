# Kafka Client Design Specification

This document provides a design specification for building Kafka producer and consumer clients that are interoperable across different programming languages.

## 1. Environment Configuration

Clients should use the following environment variables for configuration. Default values are updated to support the Dual Listener setup (Local vs Docker).

| Variable Name | Description | Default Value (Local) | Docker Value |
| :--- | :--- | :--- | :--- |
| `KAFKA_BOOTSTRAP_SERVERS` | Comma-separated list of Kafka brokers. | `127.0.0.1:9094` | `kafka-broker:9092` |
| `TOPIC_NAME` | The Kafka topic to produce to or consume from. | `my-topic-1` | `my-topic-1` |
| `CONSUMER_GROUP_ID` | The consumer group ID (Consumer only). | `my-group-1` | `my-group-1` |
| `AUTO_OFFSET_RESET` | Start position if no offset is committed. | `latest` | `latest` |

## 2. Message Schema

Messages are exchanged in JSON format. The schema is as follows:

```json
{
  "message_id": 123,
  "event_time": 1707744000.123,
  "content": "Message 123"
}
```

### Fields:
- `message_id` (Integer/Long): A monotonically increasing identifier for the message.
- `event_time` (Double): The Unix timestamp (**seconds** since epoch, e.g. `1707744000.123`) when the message was generated. *Note: Ensure seconds are used, not milliseconds.*
- `content` (String): A human-readable string, typically following the pattern `"Message {message_id}"`.

## 3. Producer Design

### Core Logic:
1. **Initialization**: Initialize the Kafka producer using `KAFKA_BOOTSTRAP_SERVERS`. Configure strictly for **IPv4** if necessary (e.g., `broker.address.family` in `rdkafka` or Java properties) to avoid `localhost` IPv6 resolution issues.
2. **Serialization**: Use a JSON serializer for the message value.
3. **Loop**:
    - Generate a `message_id` starting from 0.
    - Capture the current Unix timestamp as `event_time` (in seconds).
    - Construct the message object.
    - Send the message to `TOPIC_NAME`.
    - Log the sent message to stdout/logger.
    - Increment `message_id`.
    - Wait for 2 seconds before the next iteration.
4. **Shutdown**: Handle `SIGINT` to close the producer gracefully.

### Recommended Settings:
- `message.timeout.ms`: 5000 (if supported by the client library).

## 4. Consumer Design

### Core Logic:
1. **Initialization**: Initialize the Kafka consumer using `KAFKA_BOOTSTRAP_SERVERS` and `CONSUMER_GROUP_ID`. Force **IPv4** preference.
2. **Subscription**: Subscribe to `TOPIC_NAME`.
3. **Message Processing**:
    - Continuously poll for new messages.
    - On receipt, deserialize the JSON payload.
    - Capture the `received_time` (current Unix timestamp in seconds).
    - Calculate `latency = received_time - event_time`.
    - Log the message details and the calculated latency.
4. **Shutdown**: Handle `SIGINT` (Ctrl-C) to explicitly `close()` the consumer. **Critical** for ensuring offsets are committed to the broker before exit.

### Recommended Settings:
- `auto.offset.reset`: `latest` (default).
- `enable.auto.commit`: `true`.

## 5. Development Infrastructure

### Docker Network Strategy
To support both local development and Docker-based execution without complex host networking hacks (`socat`), the project uses a **Dual Listener** broker configuration:

- **Internal Listener (Port 9092):** Used by clients running inside the `kafka-net` Docker network.
    - Advertised as: `PLAINTEXT://kafka-broker:9092`
- **External Listener (Port 9094):** Used by clients running natively on the host machine (Mac/Linux).
    - Advertised as: `EXTERNAL://localhost:9094`
    - Mapped to host port: `9094`.

### Broker Configuration Note
For single-node setups, the broker must be configured with `KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1`. Otherwise, consumer groups will fail to stabilize as they wait for non-existent replicas.

## 6. Implementation Notes for LLM Agents

When implementing these clients in a new language:
- **Libraries**: Use the standard/popular Kafka library (e.g., `kafka-python`, `rdkafka`, `kafka-clients`).
- **Logging**: Use structured logging (SLF4J, Python `logging`, Rust `tracing`) instead of `print` statements.
- **Connectivity**: explicitly handle `localhost` resolution issues on dual-stack machines by forcing IPv4 or handling connection errors robustly.
- **Build System**: Provide a `Makefile` with `run-consumer`, `run-producer`, `docker-run-consumer`, and `docker-run-producer` targets.
- **Containerization**: Provide a multi-stage `Dockerfile` (preferring Alpine) for minimal image size.
