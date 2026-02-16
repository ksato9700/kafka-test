use rdkafka::config::ClientConfig;
use rdkafka::producer::{BaseRecord, ThreadedProducer, DefaultProducerContext};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

static SEND_COUNTER: AtomicU64 = AtomicU64::new(0);

// Zig-Zag encoding for Avro Long/Int
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
    let topic = env::var("INPUT_TOPIC").unwrap_or_else(|_| "integer-list-input-avro".to_string());

    let producer: ThreadedProducer<DefaultProducerContext> = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("broker.address.family", "v4")
        .set("compression.type", "snappy")
        .set("batch.num.messages", "100000")
        .set("linger.ms", "20")
        .set("queue.buffering.max.messages", "2000000")
        .create()
        .expect("Producer creation error");

    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(5));
            let count = SEND_COUNTER.swap(0, Ordering::Relaxed);
            if count > 0 {
                println!("📤 Rust Sending Throughput: {} messages/sec", count / 5);
            }
        }
    });

    println!("🚀 Starting Manual-Encoded High-Performance Rust Producer...");

    let mut rng = SmallRng::from_entropy();
    let mut buf = Vec::with_capacity(1024);

    loop {
        buf.clear();
        let count = rng.gen_range(2..6) as i64;
        
        // Avro Array: count (varint), then elements, then 0 (varint)
        write_varint(&mut buf, count);
        for _ in 0..count {
            let n = rng.gen_range(0..100) as i64;
            write_varint(&mut buf, n);
        }
        write_varint(&mut buf, 0); // End of array

        let record: BaseRecord<(), [u8], ()> = BaseRecord::to(&topic).payload(&buf);
        let _ = producer.send(record);
        SEND_COUNTER.fetch_add(1, Ordering::Relaxed);
    }
}
