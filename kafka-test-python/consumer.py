import os
import json
import time
import logging
import socket
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
    format='%(asctime)s %(levelname)s %(message)s',
    datefmt='%Y-%m-%d %H:%M:%S'
)

# environmental variable retrieval
def main():
    bootstrap_servers = os.getenv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9092")
    if "localhost" in bootstrap_servers:
        bootstrap_servers = bootstrap_servers.replace("localhost", "127.0.0.1")

    topic = os.getenv("TOPIC_NAME", "my-topic-1")
    group_id = os.getenv("CONSUMER_GROUP_ID", "my-group-1")

    logging.info(f"🛠️ Connecting KafkaConsumer to topic '{topic}' at '{bootstrap_servers}' (group: '{group_id}')...")

    try:
        consumer = KafkaConsumer(
            topic,
            bootstrap_servers=bootstrap_servers,
            auto_offset_reset='latest',
            group_id=group_id,
            enable_auto_commit=True,
            value_deserializer=lambda v: json.loads(v.decode("utf-8"))
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
                data = message.value
                received_time = time.time()
                latency = received_time - data.get("event_time", received_time)
                logging.info(f"📨 New message: [ID={data['message_id']}] {data['content']} at {data['event_time']:.3f}")
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
