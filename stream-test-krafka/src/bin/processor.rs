use futures::stream::{FuturesUnordered, StreamExt};
use krafka::admin::{AdminClient, NewTopic};
use krafka::consumer::{AutoOffsetReset, Consumer};
use krafka::producer::{Acks, Producer, ProducerRecord};
use tokio::sync::mpsc as async_mpsc;
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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let bootstrap = env::var("BOOTSTRAP").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let num_workers: usize = env::var("NUM_WORKERS")
        .unwrap_or_else(|_| "8".to_string())
        .parse()
        .expect("NUM_WORKERS must be a number");

    if env::var("BENCHMARK").is_ok() && env::var("SKIP_LOAD").is_err() {
        println!("🚀 Phase 1: Loading {} records (krafka)...", TOTAL_RECORDS);
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let admin = AdminClient::builder()
                .bootstrap_servers(&bootstrap)
                .build()
                .await
                .expect("Failed to create admin client");
            // Delete then recreate to ensure correct partition count.
            let _ = admin.delete_topics(
                vec![
                    "integer-list-input-benchmark".to_string(),
                    "integer-sum-output-benchmark".to_string(),
                ],
                Duration::from_secs(10),
            ).await;
            // Brief pause for deletion to propagate before recreating.
            tokio::time::sleep(Duration::from_secs(1)).await;
            admin.create_topics(
                vec![
                    NewTopic::new("integer-list-input-benchmark", num_workers as i32, 1)
                        .expect("invalid topic spec"),
                    NewTopic::new("integer-sum-output-benchmark", num_workers as i32, 1)
                        .expect("invalid topic spec"),
                ],
                Duration::from_secs(10),
                false,
            ).await.expect("Failed to create benchmark topics");

            // Wait until all partitions have elected leaders before proceeding.
            // Auto-created topics briefly return LeaderNotAvailable; polling
            // describe_topics until all partitions report a valid leader_id ensures
            // the consumer metadata cache sees a complete partition map on first refresh.
            println!("Waiting for partition leaders...");
            loop {
                let Ok(descriptions) = admin.describe_topics(&[
                    "integer-list-input-benchmark".to_string(),
                    "integer-sum-output-benchmark".to_string(),
                ]).await else {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                };
                let all_ready = descriptions.iter().all(|td| {
                    td.partitions.len() == num_workers
                        && td.partitions.iter().all(|p| p.1.leader >= 0)
                });
                if all_ready {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            println!("All partitions ready.");
            admin.close().await;

            let producer = Arc::new(
                Producer::builder()
                    .bootstrap_servers(&bootstrap)
                    .acks(Acks::None)
                    .idempotent(false)
                    .linger(Duration::from_millis(5))
                    .batch_size(64 * 1024)
                    .build()
                    .await
                    .expect("Failed to create producer"),
            );

            let mut rng = SmallRng::from_entropy();
            let mut buf = Vec::with_capacity(64);
            let mut join_set: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
            // Keep up to 10k sends in flight simultaneously (fire-and-forget style)
            const WINDOW: usize = 10_000;

            for _ in 0..TOTAL_RECORDS {
                buf.clear();
                let count = rng.gen_range(2i64..6);
                write_varint(&mut buf, count);
                for _ in 0..count {
                    write_varint(&mut buf, rng.gen_range(0i64..100));
                }
                write_varint(&mut buf, 0);

                let p = Arc::clone(&producer);
                let data = buf.clone();
                join_set.spawn(async move {
                    let _ = p.send("integer-list-input-benchmark", None, &data).await;
                });

                // Drain oldest completed send when window is full
                if join_set.len() >= WINDOW {
                    join_set.join_next().await;
                }
            }

            // Wait for all in-flight sends to complete
            while join_set.join_next().await.is_some() {}

            println!("Flushing producer...");
            producer.flush().await.expect("Failed to flush producer");
            println!("✅ Phase 1 complete.");
        });
    }

    println!("🚀 Phase 2: Processing with {} workers...", num_workers);

    let (p2_input, p2_output, is_benchmark) = if env::var("BENCHMARK").is_ok() {
        ("integer-list-input-benchmark", "integer-sum-output-benchmark", true)
    } else {
        ("integer-list-input", "integer-sum-output", false)
    };

    let reporter_stop = Arc::new(AtomicU64::new(0));
    let reporter_stop_clone = Arc::clone(&reporter_stop);

    let reporter = std::thread::spawn(move || {
        while reporter_stop_clone.load(Ordering::Relaxed) == 0 {
            std::thread::sleep(Duration::from_secs(1));
            let c = PROCESSED_COUNTER.load(Ordering::Relaxed);
            if c > 0 {
                if is_benchmark {
                    eprintln!("Progress: {}% ({} / {})", c * 100 / TOTAL_RECORDS, c, TOTAL_RECORDS);
                } else {
                    eprintln!("Progress: {} records processed", c);
                }
            }
        }
    });

    let mut handles = Vec::with_capacity(num_workers);

    for worker_id in 0..num_workers {
        let bootstrap = bootstrap.clone();
        let in_topic = p2_input.to_string();
        let out_topic = p2_output.to_string();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create worker runtime");
            rt.block_on(async move {
                let producer = Producer::builder()
                    .bootstrap_servers(&bootstrap)
                    .acks(Acks::None)
                    .idempotent(false)
                    .linger(Duration::from_millis(5))
                    .batch_size(64 * 1024)
                    .build()
                    .await
                    .expect("Failed to create worker producer");

                let consumer = Consumer::builder()
                    .bootstrap_servers(&bootstrap)
                    .auto_offset_reset(AutoOffsetReset::Earliest)
                    .max_poll_records(50_000)
                    .max_partition_fetch_bytes(10_000_000)
                    .build()
                    .await
                    .expect("Failed to create worker consumer");

                // Each worker owns exactly one partition — no group coordinator,
                // no rebalances, no heartbeat timeouts.
                // assign() internally applies auto_offset_reset (Earliest), so
                // no explicit seek is needed.
                consumer
                    .assign(&in_topic, vec![worker_id as i32])
                    .await
                    .expect("Failed to assign partition");
                consumer
                    .seek(&in_topic, worker_id as i32, 0)
                    .await
                    .expect("Failed to seek to offset 0");

                // Channel decouples the consume loop from producer I/O,
                // mirroring rdkafka's background-thread model in async.
                let (tx, mut rx) = async_mpsc::unbounded_channel::<Vec<u8>>();

                // Produce task: keeps a FuturesUnordered pool of in-flight
                // sends so the consume loop is never blocked on I/O.
                let produce_task = tokio::spawn(async move {
                    let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
                    let mut local_counter = 0u64;
                    let mut channel_open = true;

                    while channel_open || !in_flight.is_empty() {
                        tokio::select! {
                            // Accept new buffers while the channel is open
                            maybe_buf = rx.recv(), if channel_open => {
                                match maybe_buf {
                                    Some(buf) => {
                                        in_flight.push(producer.send_record(
                                        ProducerRecord::new(&*out_topic, buf),
                                    ));
                                    }
                                    None => { channel_open = false; }
                                }
                            }
                            // Drive completed sends
                            Some(_) = in_flight.next(), if !in_flight.is_empty() => {
                                local_counter += 1;
                                if local_counter >= 1000 {
                                    PROCESSED_COUNTER.fetch_add(local_counter, Ordering::Relaxed);
                                    local_counter = 0;
                                }
                            }
                        }
                    }
                    if local_counter > 0 {
                        PROCESSED_COUNTER.fetch_add(local_counter, Ordering::Relaxed);
                    }
                });

                // Consume task: polls and processes records, sends output
                // buffers to the produce task without waiting for I/O.
                let mut consecutive_empty = 0u32;
                loop {
                    if PROCESSED_COUNTER.load(Ordering::Relaxed) >= TOTAL_RECORDS {
                        break;
                    }

                    // poll() sends one fetch request with max_wait_ms=50ms.
                    // Unlike batch_recv, the outer deadline doesn't constrain
                    // how many records we can accumulate across calls.
                    let records = match consumer.poll(Duration::from_millis(50)).await {
                        Ok(r) => r,
                        Err(_) => continue,
                    };

                    if records.is_empty() {
                        consecutive_empty += 1;
                        // Break after 3 consecutive empty fetches: partition is drained.
                        if consecutive_empty >= 3 {
                            break;
                        }
                        continue;
                    }

                    consecutive_empty = 0;

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

                    for record in &records {
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
                            let mut buf = Vec::with_capacity(16);
                            write_varint(&mut buf, sum);
                            let _ = tx.send(buf);
                        }
                    }
                }

                drop(tx); // signal produce task to drain and exit
                let _ = produce_task.await;
            });
        });

        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }

    reporter_stop.store(1, Ordering::Relaxed);
    let _ = reporter.join();

    if is_benchmark {
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
