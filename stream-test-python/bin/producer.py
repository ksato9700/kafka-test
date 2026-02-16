import os
import time
import random
import multiprocessing
from confluent_kafka import Producer, KafkaException

def write_varint(n):
    val = (n << 1) ^ (n >> 63)
    buf = bytearray()
    while val >= 0x80:
        buf.append((val & 0x7f) | 0x80)
        val >>= 7
    buf.append(val)
    return buf

def producer_worker(worker_id, bootstrap_servers, topic):
    conf = {
        'bootstrap.servers': bootstrap_servers,
        'compression.type': 'snappy',
        'linger.ms': 20,
        'batch.num.messages': 100000,
        'broker.address.family': 'v4',
        'queue.buffering.max.messages': 1000000
    }
    producer = Producer(conf)
    print(f"🚀 Producer process {worker_id} (PID: {os.getpid()}) started")

    count_local = 0
    last_report = time.time()

    try:
        while True:
            list_count = random.randint(2, 5)
            buf = write_varint(list_count)
            for _ in range(list_count):
                buf += write_varint(random.randint(0, 99))
            buf += write_varint(0) # End of array

            # Handle Queue Full error
            while True:
                try:
                    producer.produce(topic, value=bytes(buf))
                    break
                except BufferError:
                    producer.poll(0.1)
                except Exception as e:
                    print(f"Produce error: {e}")
                    break
            
            producer.poll(0)
            count_local += 1
            if count_local >= 10000:
                now = time.time()
                elapsed = now - last_report
                if elapsed >= 5:
                    print(f"📤 PID {os.getpid()} Sending Throughput: {int(count_local / elapsed)} messages/sec")
                    count_local = 0
                    last_report = now
    finally:
        producer.flush()

if __name__ == "__main__":
    bootstrap_servers = os.getenv('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
    topic = os.getenv('INPUT_TOPIC', 'integer-list-input-avro')
    num_producers = int(os.getenv('PRODUCER_THREADS', '4'))

    print(f"🚀 Starting Multi-Process Python Producer with {num_producers} processes...")

    processes = []
    for i in range(num_producers):
        p = multiprocessing.Process(target=producer_worker, args=(i, bootstrap_servers, topic))
        p.start()
        processes.append(p)

    for p in processes:
        p.join()
