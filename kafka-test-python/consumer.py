import os
import json
import time
from kafka import KafkaConsumer
from kafka.errors import KafkaError

# environmental variable retrieval
def main():
    bootstrap_servers = os.getenv("KAFKA_BOOTSTRAP_SERVERS", "localhost:9092")
    topic = os.getenv("TOPIC_NAME", "my-topic-1")
    group_id = os.getenv("CONSUMER_GROUP_ID", "my-group-1")

    print(f"🛠️ Connecting KafkaConsumer to topic '{topic}' at '{bootstrap_servers}' (group: '{group_id}')...")

    try:
        consumer = KafkaConsumer(
            topic,
            bootstrap_servers=bootstrap_servers,
            auto_offset_reset='latest',
            group_id=group_id,
            enable_auto_commit=True,
            value_deserializer=lambda v: json.loads(v.decode("utf-8"))
        )
        print("✅ KafkaConsumer connected. Waiting for new messages...\n")
    except KafkaError as e:
        print(f"❌ KafkaConsumer error: {type(e).__name__} - {e}")
        exit(1)
    except Exception as e:
        print(f"❌ Unexpected error: {type(e).__name__} - {e}")
        exit(1)

    # accept messages from the topic
    print("📥 Listening for messages...")
    for message in consumer:
        try:
            data = message.value
            received_time = time.time()
            latency = received_time - data.get("event_time", received_time)
            print(f"📨 New message: [ID={data['message_id']}] {data['content']} at {data['event_time']:.3f}")
            print(f"⏱️ Latency: {latency:.3f} seconds\n")
        except Exception as e:
            print(f"⚠️ Message processing error: {type(e).__name__} - {e}")


if __name__ == "__main__":
    main()
