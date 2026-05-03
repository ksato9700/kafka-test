use krafka::consumer::{AutoOffsetReset, Consumer};
use krafka::producer::Producer;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static PROCESSED_COUNTER: AtomicU64 = AtomicU64::new(0);
static START_NANOS: AtomicU64 = AtomicU64::new(0);
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
        if shift >= 64 { break; }
    }
    ((val >> 1) as i64) ^ -((val & 1) as i64)
}

#[inline(always)]
fn write_varint(buf: &mut Vec<u8>, n: i64) {
    let mut val = ((n << 1) ^ (n >> 63)) as u64;
    while val >= 0x80 {
        buf.push((val as u8) | 0x80);
        val >>= 7;
    }
    buf.push(val as u8);
}

fn main() {
    env_logger::init();
    let bootstrap = env::var("BOOTSTRAP").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let input_topic = "integer-list-input-benchmark";
    let output_topic = "integer-sum-output-benchmark";
    let num_workers: usize = env::var("NUM_WORKERS")
        .unwrap_or_else(|_| "8".to_string())
        .parse()
        .expect("NUM_WORKERS must be a number");

    if env::var("BENCHMARK").is_ok() {
        println!("🚀 Phase 1: Loading {} records (krafka)...", TOTAL_RECORDS);
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let producer = Producer::builder()
                .bootstrap_servers(&bootstrap)
                .build()
                .await
                .expect("Failed to create producer");

            let mut rng = SmallRng::from_entropy();
            let mut buf = Vec::with_capacity(64);

            for _ in 0..TOTAL_RECORDS {
                buf.clear();
                let count = rng.gen_range(2i64..6);
                write_varint(&mut buf, count);
                for _ in 0..count {
                    write_varint(&mut buf, rng.gen_range(0i64..100));
                }
                write_varint(&mut buf, 0);
                let _ = producer.send(input_topic, None, &buf).await;
            }

            println!("Flushing producer...");
            producer.flush().await.expect("Failed to flush producer");
            println!("✅ Phase 1 complete.");
        });
    }

    println!("🚀 Phase 2: Processing with {} workers...", num_workers);

    let group_id = format!(
        "benchmark-krafka-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );

    let reporter_stop = Arc::new(AtomicU64::new(0));
    let reporter_stop_clone = Arc::clone(&reporter_stop);

    let reporter = std::thread::spawn(move || {
        while reporter_stop_clone.load(Ordering::Relaxed) == 0 {
            std::thread::sleep(Duration::from_secs(1));
            let c = PROCESSED_COUNTER.load(Ordering::Relaxed);
            if c > 0 {
                eprintln!("Progress: {}% ({} / {})", c * 100 / TOTAL_RECORDS, c, TOTAL_RECORDS);
            }
        }
    });

    let mut handles = Vec::with_capacity(num_workers);

    for _ in 0..num_workers {
        let bootstrap = bootstrap.clone();
        let group = group_id.clone();
        let in_topic = input_topic.to_string();
        let out_topic = output_topic.to_string();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create worker runtime");
            rt.block_on(async move {
                let producer = Producer::builder()
                    .bootstrap_servers(&bootstrap)
                    .build()
                    .await
                    .expect("Failed to create worker producer");

                let consumer = Consumer::builder()
                    .bootstrap_servers(&bootstrap)
                    .group_id(&group)
                    .auto_offset_reset(AutoOffsetReset::Earliest)
                    .enable_auto_commit(true)
                    .build()
                    .await
                    .expect("Failed to create worker consumer");

                consumer
                    .subscribe(&[in_topic.as_str()])
                    .await
                    .expect("Failed to subscribe");

                let mut out_buf = Vec::with_capacity(32);
                let mut local_counter = 0u64;

                loop {
                    if PROCESSED_COUNTER.load(Ordering::Relaxed) >= TOTAL_RECORDS {
                        return;
                    }

                    match consumer.poll(Duration::from_millis(100)).await {
                        Ok(records) => {
                            for record in records {
                                // Record start time on first message
                                if START_NANOS.load(Ordering::Relaxed) == 0 {
                                    let now = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap()
                                        .as_nanos() as u64;
                                    let _ = START_NANOS.compare_exchange(
                                        0,
                                        now,
                                        Ordering::SeqCst,
                                        Ordering::Relaxed,
                                    );
                                }

                                if let Some(ref value_bytes) = record.value {
                                    let mut payload: &[u8] = value_bytes.as_ref();
                                    let mut sum: i64 = 0;
                                    let mut count = read_varint(&mut payload);
                                    while count != 0 {
                                        for _ in 0..count {
                                            sum += read_varint(&mut payload);
                                        }
                                        count = read_varint(&mut payload);
                                    }

                                    out_buf.clear();
                                    write_varint(&mut out_buf, sum);
                                    let _ = producer.send(&out_topic, None, &out_buf).await;

                                    local_counter += 1;
                                    if local_counter >= 1000 {
                                        let prev = PROCESSED_COUNTER
                                            .fetch_add(local_counter, Ordering::Relaxed);
                                        if prev + local_counter >= TOTAL_RECORDS {
                                            return;
                                        }
                                        local_counter = 0;
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            });
        });

        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }

    reporter_stop.store(1, Ordering::Relaxed);
    let _ = reporter.join();

    let end_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let start = START_NANOS.load(Ordering::Relaxed);
    let duration_secs = if start > 0 {
        (end_nanos - start) as f64 / 1_000_000_000.0
    } else {
        1.0
    };

    println!("\n🏁 BENCHMARK RESULT (KRAFKA) 🏁");
    println!("Total Records:   {}", TOTAL_RECORDS);
    println!("Processing Time: {:.2}s", duration_secs);
    println!(
        "Throughput:      {:.0} msg/sec",
        TOTAL_RECORDS as f64 / duration_secs
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(n: i64) -> i64 {
        let mut buf = Vec::new();
        write_varint(&mut buf, n);
        let mut slice: &[u8] = &buf;
        read_varint(&mut slice)
    }

    #[test]
    fn test_zero() {
        assert_eq!(roundtrip(0), 0);
    }

    #[test]
    fn test_positive() {
        assert_eq!(roundtrip(1), 1);
        assert_eq!(roundtrip(100), 100);
        assert_eq!(roundtrip(127), 127);
        assert_eq!(roundtrip(128), 128);
        assert_eq!(roundtrip(300), 300);
        assert_eq!(roundtrip(i64::MAX), i64::MAX);
    }

    #[test]
    fn test_negative() {
        assert_eq!(roundtrip(-1), -1);
        assert_eq!(roundtrip(-64), -64);
        assert_eq!(roundtrip(-1000), -1000);
        assert_eq!(roundtrip(i64::MIN), i64::MIN);
    }

    #[test]
    fn test_multi_value_sequence() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 3);
        write_varint(&mut buf, 10);
        write_varint(&mut buf, 20);
        write_varint(&mut buf, 30);
        write_varint(&mut buf, 0);

        let mut slice: &[u8] = &buf;
        let count = read_varint(&mut slice);
        assert_eq!(count, 3);
        let mut sum = 0i64;
        for _ in 0..count { sum += read_varint(&mut slice); }
        let terminator = read_varint(&mut slice);
        assert_eq!(sum, 60);
        assert_eq!(terminator, 0);
    }
}
