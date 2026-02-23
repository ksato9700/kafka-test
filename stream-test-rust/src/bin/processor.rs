use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::message::Message;
use rdkafka::producer::{BaseRecord, ThreadedProducer, DefaultProducerContext, Producer};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

static PROCESSED_COUNTER: AtomicU64 = AtomicU64::new(0);
const TOTAL_RECORDS: u64 = 50_000_000;

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
    let input_topic = "integer-list-input-benchmark";
    let output_topic = "integer-sum-output-benchmark";
    let num_workers: usize = env::var("NUM_WORKERS").unwrap_or_else(|_| "8".to_string()).parse().unwrap();

    if env::var("BENCHMARK").is_ok() {
        println!("🚀 Phase 1: Loading {} records (Rust)...", TOTAL_RECORDS);
        let producer: ThreadedProducer<DefaultProducerContext> = ClientConfig::new()
            .set("bootstrap.servers", &bootstrap_servers)
            .set("broker.address.family", "v4")
            .set("compression.type", "snappy")
            .set("batch.num.messages", "100000")
            .set("linger.ms", "20")
            .create().unwrap();
        
        let mut rng = SmallRng::from_entropy();
        let mut buf = Vec::with_capacity(64);
        for _ in 0..TOTAL_RECORDS {
            buf.clear();
            let count = rng.gen_range(2..6) as i64;
            write_varint(&mut buf, count);
            for _ in 0..count { write_varint(&mut buf, rng.gen_range(0..100)); }
            write_varint(&mut buf, 0);
            let _ = producer.send(BaseRecord::<(), [u8], ()>::to(input_topic).payload(&buf));
        }
        println!("Flushing producer...");
        producer.flush(Duration::from_secs(60)).unwrap();
        println!("✅ Phase 1 Complete.");
    }

    println!("🚀 Phase 2: Processing backlog with {} workers...", num_workers);
    let group_id = format!("benchmark-rust-{}", Instant::now().elapsed().as_nanos());
    
    let mut handles = Vec::new();
    let start_instant = Arc::new(AtomicU64::new(0));

    for _i in 0..num_workers {
        let bootstrap = bootstrap_servers.clone();
        let group = group_id.clone();
        let in_topic = input_topic.to_string();
        let out_topic = output_topic.to_string();
        let start_time_ref = Arc::clone(&start_instant);

        let handle = std::thread::spawn(move || {
            let producer: ThreadedProducer<DefaultProducerContext> = ClientConfig::new()
                .set("bootstrap.servers", &bootstrap)
                .set("broker.address.family", "v4")
                .set("compression.type", "snappy")
                .set("batch.num.messages", "100000")
                .set("linger.ms", "20")
                .create().unwrap();

            let consumer: BaseConsumer = ClientConfig::new()
                .set("bootstrap.servers", &bootstrap)
                .set("broker.address.family", "v4")
                .set("group.id", &group)
                .set("auto.offset.reset", "earliest")
                .set("fetch.min.bytes", "1000000")
                .set("fetch.message.max.bytes", "10000000")
                .set("queued.min.messages", "2000000")
                .create().unwrap();
            
            consumer.subscribe(&[&in_topic]).unwrap();
            let mut out_buf = Vec::with_capacity(32);
            let mut local_counter = 0;
            
            loop {
                if let Some(Ok(m)) = consumer.poll(Duration::from_millis(10)) {
                    if start_time_ref.load(Ordering::Relaxed) == 0 {
                        let _ = start_time_ref.compare_exchange(0, 1, Ordering::SeqCst, Ordering::Relaxed);
                    }

                    if let Some(mut payload) = m.payload() {
                        let mut sum: i64 = 0;
                        let mut count = read_varint(&mut payload);
                        while count != 0 {
                            let abs_count = count.abs();
                            if count < 0 { let _ = read_varint(&mut payload); }
                            for _ in 0..abs_count { sum += read_varint(&mut payload); }
                            count = read_varint(&mut payload);
                        }
                        out_buf.clear();
                        write_varint(&mut out_buf, sum);
                        let _ = producer.send(BaseRecord::to(&out_topic).payload(out_buf.as_slice()).key(m.key().unwrap_or(&[])));
                        
                        local_counter += 1;
                        if local_counter >= 1000 {
                            if PROCESSED_COUNTER.fetch_add(local_counter, Ordering::Relaxed) + local_counter >= TOTAL_RECORDS {
                                break;
                            }
                            local_counter = 0;
                        }
                    }
                }
                if PROCESSED_COUNTER.load(Ordering::Relaxed) >= TOTAL_RECORDS { break; }
            }
        });
        handles.push(handle);
    }

    let overall_start = Instant::now();
    let reporter_stop = Arc::new(AtomicU64::new(0));
    let reporter_stop_clone = Arc::clone(&reporter_stop);
    let reporter = std::thread::spawn(move || {
        while PROCESSED_COUNTER.load(Ordering::Relaxed) < TOTAL_RECORDS && reporter_stop_clone.load(Ordering::Relaxed) == 0 {
            std::thread::sleep(Duration::from_secs(1));
            let c = PROCESSED_COUNTER.load(Ordering::Relaxed);
            if c > 0 { println!("Progress: {}% ({} / {})", c * 100 / TOTAL_RECORDS, c, TOTAL_RECORDS); }
        }
    });

    for h in handles { let _ = h.join(); }
    let duration = overall_start.elapsed();
    reporter_stop.store(1, Ordering::Relaxed);
    let _ = reporter.join();
    
    println!("🏁 BENCHMARK RESULT (RUST) 🏁");
    println!("Total Records: {}", TOTAL_RECORDS);
    println!("Processing Time: {:.2}s", duration.as_secs_f64());
    println!("Throughput:      {:.0} msg/sec", TOTAL_RECORDS as f64 / duration.as_secs_f64());
}
