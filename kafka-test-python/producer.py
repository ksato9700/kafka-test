import io
import json
import logging
import os
import socket
import time
from pathlib import Path

import fastavro
from kafka import KafkaProducer

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


def main():
    logging.info("Hello from kafka-test-python!")

    # Load Avro schema
    schema = load_schema()
    logging.info(f"Loaded Avro schema from {schema['name']}")

    # environmental variable retrieval
    bootstrap_servers = os.getenv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
    if "localhost" in bootstrap_servers:
        bootstrap_servers = bootstrap_servers.replace("localhost", "127.0.0.1")

    topic = os.getenv("TOPIC_NAME", "my-topic-1")

    # KafkaProducer initialization
    # We serialize manually to handle Avro
    producer = KafkaProducer(
        bootstrap_servers=bootstrap_servers,
    )

    logging.info("🚀 Producer is now running...")

    message_id = 0
    while True:
        event_time = time.time()
        message = {
            "message_id": message_id,
            "event_time": event_time,
            "content": f"Message {message_id}",
        }

        # Serialize to Avro (schemaless)
        bytes_io = io.BytesIO()
        fastavro.schemaless_writer(bytes_io, schema, message)
        raw_bytes = bytes_io.getvalue()

        producer.send(topic, value=raw_bytes)
        logging.info(f"🚀 Sent: {message}")

        message_id += 1
        time.sleep(2)  # send message every 2 sec.


if __name__ == "__main__":
    main()
