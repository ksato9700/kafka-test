import os
import time
import multiprocessing
from confluent_kafka import Consumer, Producer, KafkaError

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

def write_varint(n):
    val = (n << 1) ^ (n >> 63)
    buf = bytearray()
    while val >= 0x80:
        buf.append((val & 0x7f) | 0x80)
        val >>= 7
    buf.append(val)
    return buf

def worker(worker_id, bootstrap_servers, input_topic, output_topic, group_id):
    consumer_conf = {
        'bootstrap.servers': bootstrap_servers,
        'group.id': group_id,
        'auto.offset.reset': 'earliest',
        'enable.auto.commit': True,
        'fetch.min.bytes': 100000,
        'broker.address.family': 'v4'
    }
    producer_conf = {
        'bootstrap.servers': bootstrap_servers,
        'compression.type': 'snappy',
        'linger.ms': 20,
        'batch.num.messages': 100000,
        'broker.address.family': 'v4',
        'queue.buffering.max.messages': 1000000
    }

    consumer = Consumer(consumer_conf)
    producer = Producer(producer_conf)
    consumer.subscribe([input_topic])

    print(f"🚀 Worker process {worker_id} (PID: {os.getpid()}) started")

    msg_count = 0
    last_report = time.time()

    try:
        while True:
            msg = consumer.poll(0.1)
            if msg is None:
                continue
            if msg.error():
                if msg.error().code() == KafkaError._PARTITION_EOF:
                    continue
                else:
                    print(f"Worker {worker_id} error: {msg.error()}")
                    break

            payload = msg.value()
            pos = 0
            
            # Decode IntegerList
            count, pos = read_varint(payload, pos)
            total_sum = 0
            while count != 0:
                abs_count = abs(count)
                if count < 0:
                    _, pos = read_varint(payload, pos) # skip byte size
                for _ in range(abs_count):
                    val, pos = read_varint(payload, pos)
                    total_sum += val
                count, pos = read_varint(payload, pos)

            # Encode SumResult
            out_payload = bytes(write_varint(total_sum))
            
            # Handle Queue Full error
            while True:
                try:
                    producer.produce(output_topic, value=out_payload, key=msg.key())
                    break
                except BufferError:
                    producer.poll(0.1)
            
            producer.poll(0)
            msg_count += 1
            if msg_count >= 10000:
                now = time.time()
                elapsed = now - last_report
                if elapsed >= 5:
                    print(f"🚀 PID {os.getpid()} Throughput: {int(msg_count / elapsed)} messages/sec")
                    msg_count = 0
                    last_report = now
    finally:
        consumer.close()
        producer.flush()

if __name__ == "__main__":
    bootstrap_servers = os.getenv('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
    input_topic = os.getenv('INPUT_TOPIC', 'integer-list-input-avro')
    output_topic = os.getenv('OUTPUT_TOPIC', 'integer-sum-output-avro')
    group_id = os.getenv('APPLICATION_ID', 'stream-sum-test-python-avro')
    num_workers = int(os.getenv('NUM_WORKERS', '4'))

    print(f"🚀 Starting Multi-Process Python Stream Processor with {num_workers} processes...")

    processes = []
    for i in range(num_workers):
        p = multiprocessing.Process(target=worker, args=(i, bootstrap_servers, input_topic, output_topic, group_id))
        p.start()
        processes.append(p)

    for p in processes:
        p.join()
