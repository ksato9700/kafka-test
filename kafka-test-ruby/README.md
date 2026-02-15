# Kafka Test Ruby

A Ruby implementation of the Kafka producer and consumer clients.

## Requirements

- Ruby 3.x
- [librdkafka](https://github.com/confluentinc/librdkafka) installed on your system.

## Setup

1. Install dependencies:
   ```bash
   bundle install
   ```

## Running the Producer

```bash
export KAFKA_BOOTSTRAP_SERVERS="localhost:9092"
export TOPIC_NAME="my-topic-1"
bundle exec ruby producer.rb
```

## Running the Consumer

```bash
export KAFKA_BOOTSTRAP_SERVERS="localhost:9092"
export TOPIC_NAME="my-topic-1"
export CONSUMER_GROUP_ID="my-group-1"
bundle exec ruby consumer.rb
```
