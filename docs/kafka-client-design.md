# Kafka Client Design Specification

This document provides a design specification for building Kafka producer and consumer clients that are interoperable across different programming languages.

## 1. Environment Configuration

Clients should use the following environment variables for configuration. Default values are provided for local development.

| Variable Name | Description | Default Value |
| :--- | :--- | :--- |
| `KAFKA_BOOTSTRAP_SERVERS` | Comma-separated list of Kafka brokers. | `localhost:9092` |
| `TOPIC_NAME` | The Kafka topic to produce to or consume from. | `my-topic-1` |
| `CONSUMER_GROUP_ID` | The consumer group ID (Consumer only). | `my-group-1` |

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
- `event_time` (Float/Double): The Unix timestamp (seconds since epoch) when the message was generated.
- `content` (String): A human-readable string, typically following the pattern `"Message {message_id}"`.

## 3. Producer Design

### Core Logic:
1. **Initialization**: Initialize the Kafka producer using `KAFKA_BOOTSTRAP_SERVERS`.
2. **Serialization**: Use a JSON serializer for the message value.
3. **Loop**:
    - Generate a `message_id` starting from 0.
    - Capture the current Unix timestamp as `event_time`.
    - Construct the message object.
    - Send the message to `TOPIC_NAME`.
    - Log the sent message to stdout.
    - Increment `message_id`.
    - Wait for 2 seconds before the next iteration.

### Recommended Settings:
- `message.timeout.ms`: 5000 (if supported by the client library).

## 4. Consumer Design

### Core Logic:
1. **Initialization**: Initialize the Kafka consumer using `KAFKA_BOOTSTRAP_SERVERS` and `CONSUMER_GROUP_ID`.
2. **Subscription**: Subscribe to `TOPIC_NAME`.
3. **Message Processing**:
    - Continuously poll for new messages.
    - On receipt, deserialize the JSON payload.
    - Capture the `received_time` (current Unix timestamp).
    - Calculate `latency = received_time - event_time`.
    - Log the message details and the calculated latency to stdout.

### Recommended Settings:
- `auto.offset.reset`: `latest` (Start consuming from the end of the topic if no offset is committed).
- `enable.auto.commit`: `true`.

## 5. Implementation Notes for LLM Agents

When implementing these clients in a new language:
- **Libraries**: Use the most popular and well-supported Kafka library for that language (e.g., `confluent-kafka` for Python/Go/C#, `rdkafka` for Rust/C++, `kafka-clients` for Java).
- **Error Handling**: Implement basic error handling for connection issues and serialization/deserialization failures.
- **Asynchronous IO**: If the language supports it (like Node.js, Python with `aiokafka`, or Rust with `tokio`), prefer asynchronous implementations for better performance.
