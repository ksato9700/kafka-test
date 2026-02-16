use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::message::Message;
use rdkafka::producer::{BaseRecord, ThreadedProducer, DefaultProducerContext};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

static MESSAGE_COUNTER: AtomicU64 = AtomicU64::new(0);

// Optimized Zig-Zag decoding
#[inline(always)]
fn read_varint(reader: &mut &[u8]) -> i64 {
    let mut val: u64 = 0;
    let mut shift = 0;
    loop {
        let b = reader[0];
        *reader = &(*reader)[1..];
        val |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 { break; }
        shift += 7;
    }
    ((val >> 1) as i64) ^ -((val & 1) as i64)
}

// Optimized Zig-Zag encoding
#[inline(always)]
fn write_varint(buf: &mut Vec<u8>, n: i64) {
    let mut val = (n << 1) ^ (n >> 63);
    while val >= 0x80 {
        buf.push((val as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

fn main() {
    let bootstrap_servers = env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let input_topic = env::var("INPUT_TOPIC").unwrap_or_else(|_| "integer-list-input-avro".to_string());
    let output_topic = env::var("OUTPUT_TOPIC").unwrap_or_else(|_| "integer-sum-output-avro".to_string());
    let group_id = env::var("APPLICATION_ID").unwrap_or_else(|_| "stream-sum-test-rust-avro".to_string());
    let num_workers: usize = env::var("NUM_WORKERS").unwrap_or_else(|_| "3".to_string()).parse().unwrap();

    // Throughput reporter
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let count = MESSAGE_COUNTER.swap(0, Ordering::Relaxed);
            if count > 0 {
                println!("🚀 Rust Throughput: {} messages/sec", count / 5);
            }
        }
    });

    println!("🚀 Starting Ultra-Performance Rust Stream Processor with {} workers...", num_workers);

    let mut handles = Vec::new();
    for i in 0..num_workers {
        let bootstrap = bootstrap_servers.clone();
        let group = group_id.clone();
        let in_topic = input_topic.clone();
        let out_topic = output_topic.clone();

        let handle = std::thread::spawn(move || {
            // Per-worker producer to avoid cross-thread contention
            let producer: ThreadedProducer<DefaultProducerContext> = ClientConfig::new()
                .set("bootstrap.servers", &bootstrap)
                .set("broker.address.family", "v4")
                .set("compression.type", "snappy")
                .set("batch.num.messages", "100000")
                .set("linger.ms", "20")
                .set("queue.buffering.max.messages", "1000000")
                .create()
                .expect("Producer creation error");

            let consumer: BaseConsumer = ClientConfig::new()
                .set("bootstrap.servers", &bootstrap)
                .set("broker.address.family", "v4")
                .set("group.id", &group)
                .set("enable.auto.commit", "true")
                .set("auto.offset.reset", "earliest")
                .set("fetch.message.max.bytes", "10485760")
                .set("fetch.max.bytes", "52428800")
                .set("queued.min.messages", "1000000")
                // Increase fetch efficiency
                .set("fetch.min.bytes", "100000")
                .set("fetch.wait.max.ms", "100")
                .create()
                .expect("Consumer creation error");
            
            consumer.subscribe(&[&in_topic]).expect("Can't subscribe");
            
            let mut out_buf = Vec::with_capacity(64);
            let mut local_counter = 0;
            
            println!("Worker thread {} started with private producer", i);
            
            loop {
                // Using 0 timeout for tightest possible polling
                if let Some(result) = consumer.poll(Duration::from_secs(0)) {
                    match result {
                        Ok(m) => {
                            if let Some(mut payload) = m.payload() {
                                let mut sum: i64 = 0;
                                let mut count = read_varint(&mut payload);
                                while count != 0 {
                                    let abs_count = count.abs();
                                    if count < 0 { let _ = read_varint(&mut payload); }
                                    for _ in 0..abs_count {
                                        sum += read_varint(&mut payload);
                                    }
                                    count = read_varint(&mut payload);
                                }

                                out_buf.clear();
                                write_varint(&mut out_buf, sum);

                                let record = BaseRecord::to(&out_topic)
                                    .payload(out_buf.as_slice())
                                    .key(m.key().unwrap_or(&[]));
                                
                                let _ = producer.send(record);
                                
                                local_counter += 1;
                                if local_counter >= 1000 {
                                    MESSAGE_COUNTER.fetch_add(local_counter, Ordering::Relaxed);
                                    local_counter = 0;
                                }
                            }
                        }
                        Err(e) => eprintln!("Worker {} error: {}", i, e),
                    }
                }
            }
        });
        handles.push(handle);
    }

    for h in handles { let _ = h.join(); }
}
