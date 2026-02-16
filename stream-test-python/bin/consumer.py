import os
from confluent_kafka import Consumer, KafkaError

def read_varint(data, pos):
    val = 0
    shift = 0
    while True:
        b = data[pos]
        pos += 1
        val |= (b & 0x7f) << shift
        if not (b & 0x80):
            break
        shift += 7
    return (val >> 1) ^ -(val & 1), pos

def main():
    bootstrap_servers = os.getenv('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
    topic = os.getenv('OUTPUT_TOPIC', 'integer-sum-output-avro')

    conf = {
        'bootstrap.servers': bootstrap_servers,
        'group.id': 'python-test-result-consumer',
        'auto.offset.reset': 'earliest',
        'broker.address.family': 'v4'
    }

    consumer = Consumer(conf)
    consumer.subscribe([topic])

    print(f"🚀 Waiting for Avro results on topic: {topic}...")

    try:
        while True:
            msg = consumer.poll(0.1)
            if msg is None:
                continue
            if msg.error():
                if msg.error().code() == KafkaError._PARTITION_EOF:
                    continue
                else:
                    print(f"Consumer error: {msg.error()}")
                    break

            payload = msg.value()
            res_sum, _ = read_varint(payload, 0)
            print(f"Received Avro Result: sum={res_sum}")
    finally:
        consumer.close()

if __name__ == "__main__":
    main()
