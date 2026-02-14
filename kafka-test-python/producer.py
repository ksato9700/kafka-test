import os
import json
import time
import logging
import socket
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
    format='%(asctime)s %(levelname)s %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)

def main():
    logging.info("Hello from kafka-test-python!")

    # environmental variable retrieval
    bootstrap_servers = os.getenv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9092")
    if "localhost" in bootstrap_servers:
        bootstrap_servers = bootstrap_servers.replace("localhost", "127.0.0.1")

    topic = os.getenv("TOPIC_NAME", "my-topic-1")

    # KafkaProducer initialization
    producer = KafkaProducer(
        bootstrap_servers=bootstrap_servers,
        value_serializer=lambda v: json.dumps(v).encode("utf-8")
    )

    logging.info("🚀 Producer is now running...")

    message_id = 0
    while True:
        event_time = time.time()
        message = {
            "message_id": message_id,
            "event_time": event_time,
            "content": f"Message {message_id}"
        }

        producer.send(topic, value=message)
        logging.info(f"🚀 Sent: {message}")

        message_id += 1
        time.sleep(2)  # send message every 10 sec.

if __name__ == "__main__":
    main()
