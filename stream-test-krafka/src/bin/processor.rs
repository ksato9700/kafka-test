use krafka::producer::Producer;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::env;
use std::sync::atomic::{AtomicU64, Ordering};
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

    // Phase 2 will be added in Task 4
    let _ = (output_topic, num_workers);
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
