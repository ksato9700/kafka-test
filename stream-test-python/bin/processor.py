import os
import time
import random
import multiprocessing
from confluent_kafka import Consumer, Producer, KafkaError

TOTAL_RECORDS = 50000000

def read_varint(data, pos):
    val = 0
    shift = 0
    while True:
        b = data[pos]
        pos += 1
        val |= (b & 0x7f) << shift
        if not (b & 0x80): break
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

def worker(worker_id, bootstrap_servers, input_topic, output_topic, group_id, counter):
    consumer = Consumer({
        'bootstrap.servers': bootstrap_servers,
        'group.id': group_id,
        'auto.offset.reset': 'earliest',
        'enable.auto.commit': True,
        'fetch.min.bytes': 1000000,
        'broker.address.family': 'v4'
    })
    producer = Producer({
        'bootstrap.servers': bootstrap_servers,
        'compression.type': 'snappy',
        'linger.ms': 20,
        'batch.num.messages': 100000,
        'broker.address.family': 'v4'
    })
    consumer.subscribe([input_topic])

    local_count = 0
    while counter.value < TOTAL_RECORDS:
        msg = consumer.poll(0.1)
        if msg is None: continue
        if msg.error(): continue

        payload = msg.value()
        pos = 0
        count, pos = read_varint(payload, pos)
        total_sum = 0
        while count != 0:
            abs_count = abs(count)
            if count < 0: _, pos = read_varint(payload, pos)
            for _ in range(abs_count):
                val, pos = read_varint(payload, pos)
                total_sum += val
            count, pos = read_varint(payload, pos)

        producer.produce(output_topic, value=bytes(write_varint(total_sum)), key=msg.key())
        
        local_count += 1
        if local_count >= 1000:
            with counter.get_lock():
                counter.value += local_count
            local_count = 0
    
    producer.flush()
    consumer.close()

if __name__ == "__main__":
    bootstrap_servers = os.getenv('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
    input_topic = "integer-list-input-benchmark"
    output_topic = "integer-sum-output-benchmark"
    num_workers = int(os.getenv('NUM_WORKERS', '8'))

    if os.getenv("BENCHMARK"):
        print(f"🚀 Phase 1: Loading {TOTAL_RECORDS} records (Python)...")
        p = Producer({'bootstrap.servers': bootstrap_servers, 'compression.type': 'snappy', 'broker.address.family': 'v4'})
        for i in range(TOTAL_RECORDS):
            list_count = random.randint(2, 5)
            buf = write_varint(list_count)
            for _ in range(list_count): buf += write_varint(random.randint(0, 99))
            buf += write_varint(0)
            p.produce(input_topic, value=bytes(buf))
            if i % 100000 == 0: p.poll(0)
        p.flush()
        print("✅ Phase 1 Complete.")

    print(f"🚀 Phase 2: Processing backlog with {num_workers} processes...")
    group_id = f"benchmark-python-{int(time.time())}"
    counter = multiprocessing.Value('L', 0)
    
    start_time = time.time()
    processes = [multiprocessing.Process(target=worker, args=(i, bootstrap_servers, input_topic, output_topic, group_id, counter)) for i in range(num_workers)]
    for p in processes: p.start()

    while counter.value < TOTAL_RECORDS:
        time.sleep(1)
        c = counter.value
        if c > 0: print(f"Progress: {int(c * 100 / TOTAL_RECORDS)}% ({c} / {TOTAL_RECORDS})")

    for p in processes: p.join()
    duration = time.time() - start_time

    print("🏁 BENCHMARK RESULT (PYTHON) 🏁")
    print(f"Total Records: {TOTAL_RECORDS}")
    print(f"Processing Time: {duration:.2f}s")
    print(f"Throughput:      {int(TOTAL_RECORDS / duration)} msg/sec")
