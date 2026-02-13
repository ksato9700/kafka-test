import os
import json
import time
from kafka import KafkaProducer

def main():
    print("Hello from kafka-test-python!")

    # environmental variable retrieval
    bootstrap_servers = os.getenv("KAFKA_BOOTSTRAP_SERVERS", "localhost:9092")
    topic = os.getenv("TOPIC_NAME", "my-topic-1")

    # KafkaProducer initialization
    producer = KafkaProducer(
        bootstrap_servers=bootstrap_servers,
        value_serializer=lambda v: json.dumps(v).encode("utf-8")
    )

    print("🚀 Producer is now running...")

    message_id = 0
    while True:
        event_time = time.time()
        message = {
            "message_id": message_id,
            "event_time": event_time,
            "content": f"Message {message_id}"
        }

        producer.send(topic, value=message)
        print(f"🚀 Sent: {message}")

        message_id += 1
        time.sleep(2)  # send message every 10 sec.

if __name__ == "__main__":
    main()
