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

## 2. Message Schema (Avro)

Messages are exchanged in **Raw (Schemaless) Avro** format. The schema is defined in `schemas/message.avsc`:

```json
{
  "type": "record",
  "name": "Message",
  "namespace": "com.example.kafka",
  "fields": [
    {"name": "message_id", "type": "long"},
    {"name": "event_time", "type": "double"},
    {"name": "content", "type": "string"}
  ]
}
```

### Serialization Strategy:
- **Format:** Raw Avro binary (no OCF header, no Schema Registry magic byte).
- **Schema Source:** Clients must load the schema from the `schemas/message.avsc` file at runtime.
    - Local: `../schemas/message.avsc` (relative to project root).
    - Docker: `/app/schemas/message.avsc` (copied into image).

### Fields:
- `message_id` (Long): A monotonically increasing identifier.
- `event_time` (Double): Unix timestamp in **seconds** (e.g. `1707744000.123`).
- `content` (String): e.g., `"Message {message_id}"`.

## 3. Producer Design

### Core Logic:
1. **Initialization**: Initialize the Kafka producer using `KAFKA_BOOTSTRAP_SERVERS`. Configure strictly for **IPv4**.
2. **Schema Loading**: Load and parse `schemas/message.avsc`.
3. **Serialization**: Serialize the message object to **Raw Avro bytes** using the loaded schema.
4. **Loop**:
    - Generate `message_id`.
    - Capture `event_time` (seconds).
    - Construct message record.
    - Serialize to bytes.
    - Send bytes to `TOPIC_NAME`.
    - Log sent message.
    - Wait 2 seconds.
5. **Shutdown**: Handle `SIGINT`.

## 4. Consumer Design

### Core Logic:
1. **Initialization**: Initialize consumer. Force **IPv4**.
2. **Schema Loading**: Load and parse `schemas/message.avsc`.
3. **Subscription**: Subscribe to `TOPIC_NAME`.
4. **Message Processing**:
    - Poll for messages.
    - Deserialize the **Raw Avro bytes** using the schema.
    - Capture `received_time` (seconds).
    - Calculate `latency = received_time - event_time`.
    - Log details and latency.
5. **Shutdown**: Handle `SIGINT`.

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
