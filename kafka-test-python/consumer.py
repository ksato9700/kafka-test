import io
import json
import logging
import os
import socket
import time
from pathlib import Path

import fastavro
from kafka import KafkaConsumer
from kafka.errors import KafkaError

# Monkey-patch socket.getaddrinfo to force IPv4 for localhost
_orig_getaddrinfo = socket.getaddrinfo


def _getaddrinfo_hook(host, port, family=0, type=0, proto=0, flags=0):
    if host == "localhost":
        family = socket.AF_INET
    return _orig_getaddrinfo(host, port, family, type, proto, flags)


socket.getaddrinfo = _getaddrinfo_hook

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(message)s",
    datefmt="%Y-%m-%d %H:%M:%S",
)


def load_schema():
    # Try to find schema relative to current file or at /app/schemas
    base_dir = Path(__file__).resolve().parent
    schema_path_local = base_dir / "../schemas/message.avsc"
    schema_path_docker = Path("/app/schemas/message.avsc")

    path_to_use = (
        schema_path_local if schema_path_local.exists() else schema_path_docker
    )

    if not path_to_use.exists():
        raise FileNotFoundError(f"Schema not found at {path_to_use}")

    with path_to_use.open("r") as f:
        return fastavro.parse_schema(json.load(f))


# environmental variable retrieval
def main():
    # Load Avro schema
    schema = load_schema()
    logging.info(f"Loaded Avro schema from {schema['name']}")

    bootstrap_servers = os.getenv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
    if "localhost" in bootstrap_servers:
        bootstrap_servers = bootstrap_servers.replace("localhost", "127.0.0.1")

    topic = os.getenv("TOPIC_NAME", "my-topic-1")
    group_id = os.getenv("CONSUMER_GROUP_ID", "my-group-1")

    logging.info(
        f"🛠️ Connecting KafkaConsumer to topic '{topic}' at '{bootstrap_servers}' (group: '{group_id}')..."
    )

    try:
        # No value_deserializer, we handle raw bytes
        consumer = KafkaConsumer(
            topic,
            bootstrap_servers=bootstrap_servers,
            auto_offset_reset="latest",
            group_id=group_id,
            enable_auto_commit=True,
        )
        logging.info("✅ KafkaConsumer connected. Waiting for new messages...\n")
    except KafkaError as e:
        logging.error(f"❌ KafkaConsumer error: {type(e).__name__} - {e}")
        exit(1)
    except Exception as e:
        logging.error(f"❌ Unexpected error: {type(e).__name__} - {e}")
        exit(1)

    # accept messages from the topic
    logging.info("📥 Listening for messages...")
    try:
        for message in consumer:
            try:
                # Deserialize Avro
                bytes_io = io.BytesIO(message.value)
                data = fastavro.schemaless_reader(bytes_io, schema)

                received_time = time.time()
                latency = received_time - data.get("event_time", received_time)
                logging.info(
                    f"📨 New message: [ID={data['message_id']}] {data['content']} at {data['event_time']:.3f}"
                )
                logging.info(f"⏱️ Latency: {latency:.3f} seconds\n")
            except Exception as e:
                logging.warning(f"⚠️ Message processing error: {type(e).__name__} - {e}")
    except KeyboardInterrupt:
        logging.info("🛑 Shutting down gracefully...")
    finally:
        consumer.close()
        logging.info("✅ KafkaConsumer closed.")


if __name__ == "__main__":
    main()
