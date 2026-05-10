use futures::stream::{FuturesUnordered, StreamExt};
use krafka::admin::{AdminClient, NewTopic};
use krafka::consumer::{AutoOffsetReset, Consumer};
use krafka::producer::{Acks, Producer, ProducerRecord};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::env;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

static PROCESSED_COUNTER: AtomicU64 = AtomicU64::new(0);
static START_NANOS: AtomicU64 = AtomicU64::new(0);
static STOP_FLAG: AtomicBool = AtomicBool::new(false);
const TOTAL_RECORDS: u64 = 50_000_000;
const NUM_WORKERS: usize = 8;
const CHUNK_SIZE: usize = 5_000;
// Max send_record futures in-flight per worker before we pause receiving new batches.
// Keeps memory bounded while allowing fetch and send to overlap fully.
const MAX_IN_FLIGHT: usize = 5_000;

#[inline(always)]
fn read_varint(reader: &mut &[u8]) -> i64 {
    let mut val: u64 = 0;
    let mut shift = 0;
    loop {
        let b = reader[0];
        *reader = &(*reader)[1..];
        val |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            break;
        }
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

    if env::var("BENCHMARK").is_ok() && env::var("SKIP_LOAD").is_err() {
        println!("🚀 Phase 1: Loading {} records (krafka2)...", TOTAL_RECORDS);
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            let admin = AdminClient::builder()
                .bootstrap_servers(&bootstrap)
                .build()
                .await
                .expect("Failed to create admin client");

            let _ = admin
                .delete_topics(
                    vec![
                        "integer-list-input-benchmark".to_string(),
                        "integer-sum-output-benchmark".to_string(),
                    ],
                    Duration::from_secs(10),
                )
                .await;
            tokio::time::sleep(Duration::from_secs(1)).await;

            admin
                .create_topics(
                    vec![
                        NewTopic::new("integer-list-input-benchmark", NUM_WORKERS as i32, 1)
                            .expect("invalid topic spec"),
                        NewTopic::new("integer-sum-output-benchmark", NUM_WORKERS as i32, 1)
                            .expect("invalid topic spec"),
                    ],
                    Duration::from_secs(10),
                )
                .await
                .expect("Failed to create benchmark topics");

            println!("Waiting for partition leaders...");
            loop {
                let Ok(descriptions) = admin
                    .describe_topics(&[
                        "integer-list-input-benchmark".to_string(),
                        "integer-sum-output-benchmark".to_string(),
                    ])
                    .await
                else {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                };
                let all_ready = descriptions.iter().all(|td| {
                    td.partitions.len() == NUM_WORKERS
                        && td.partitions.iter().all(|p| p.leader >= 0)
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

                if join_set.len() >= WINDOW {
                    join_set.join_next().await;
                }
            }
            while join_set.join_next().await.is_some() {}

            println!("Flushing producer...");
            producer.flush().await.expect("Failed to flush producer");
            println!("✅ Phase 1 complete.");
        });
    }

    println!(
        "🚀 Phase 2: Processing (krafka2 pipeline, {} workers)...",
        NUM_WORKERS
    );

    let (in_topic, out_topic, is_benchmark) = if env::var("BENCHMARK").is_ok() {
        (
            "integer-list-input-benchmark",
            "integer-sum-output-benchmark",
            true,
        )
    } else {
        ("integer-list-input", "integer-sum-output", false)
    };

    // Async channel: process tasks use rx.recv().await which yields to the executor
    // instead of parking the OS thread, allowing send_record futures to make progress
    // while waiting for the next batch.
    let (tx, rx) = mpsc::channel::<Vec<krafka::consumer::ConsumerRecord>>(NUM_WORKERS * 4);

    let reporter_stop = Arc::new(AtomicBool::new(false));
    let reporter_stop_clone = Arc::clone(&reporter_stop);

    let reporter = std::thread::spawn(move || {
        while !reporter_stop_clone.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_secs(1));
            let c = PROCESSED_COUNTER.load(Ordering::Relaxed);
            if c > 0 {
                if is_benchmark {
                    eprintln!(
                        "Progress: {}% ({} / {})",
                        c * 100 / TOTAL_RECORDS,
                        c,
                        TOTAL_RECORDS
                    );
                } else {
                    eprintln!("Progress: {} records processed", c);
                }
            }
        }
    });

    // Spawn N process tasks — each owns its own Producer and a single-thread tokio runtime.
    // The runtime is now NOT blocked by rx.recv() (it's async), so the executor can
    // continuously interleave: receive new batches → decode → push to sends → drain sends.
    let rx = Arc::new(tokio::sync::Mutex::new(rx));
    let mut process_handles = Vec::with_capacity(NUM_WORKERS);

    for _ in 0..NUM_WORKERS {
        let rx = Arc::clone(&rx);
        let bootstrap = bootstrap.clone();
        let out_topic = out_topic.to_string();

        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build worker runtime");

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

                let mut sends: FuturesUnordered<_> = FuturesUnordered::new();
                let mut channel_open = true;

                loop {
                    tokio::select! {
                        // Receive new sub-batch when in-flight queue has headroom.
                        // rx.recv().await yields (not parks) — executor stays alive.
                        batch = async { rx.lock().await.recv().await }, if channel_open && sends.len() < MAX_IN_FLIGHT => {
                            match batch {
                                Some(records) => {
                                    if START_NANOS.load(Ordering::Relaxed) == 0 {
                                        let now = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_nanos() as u64;
                                        let _ = START_NANOS.compare_exchange(
                                            0, now, Ordering::SeqCst, Ordering::Relaxed,
                                        );
                                    }
                                    for record in records {
                                        if let Some(value_bytes) = record.value {
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
                                            sends.push(
                                                producer.send_record(ProducerRecord::new(&*out_topic, buf))
                                            );
                                        }
                                    }
                                }
                                None => { channel_open = false; }
                            }
                        }

                        // Drive one completed send future; count it.
                        Some(_) = sends.next(), if !sends.is_empty() => {
                            let prev = PROCESSED_COUNTER.fetch_add(1, Ordering::Relaxed);
                            if prev + 1 >= TOTAL_RECORDS {
                                STOP_FLAG.store(true, Ordering::Relaxed);
                            }
                        }

                        else => { break; }
                    }
                }

                producer.flush().await.ok();
            });
        });

        process_handles.push(handle);
    }

    // Fetch task: one consumer, all partitions, tight poll loop.
    // Sends sub-batches via async mpsc — tx.send().await provides backpressure
    // without blocking the OS thread (it yields to the tokio executor).
    let bootstrap_fetch = bootstrap.clone();
    let in_topic_fetch = in_topic.to_string();
    let fetch_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Failed to build fetch runtime");

        rt.block_on(async move {
            let consumer = Consumer::builder()
                .bootstrap_servers(&bootstrap_fetch)
                .auto_offset_reset(AutoOffsetReset::Earliest)
                .max_poll_records(50_000)
                .max_partition_fetch_bytes(10_000_000)
                .fetch_min_bytes(1_000_000)
                .build()
                .await
                .expect("Failed to create fetch consumer");

            let partitions: Vec<i32> = (0..NUM_WORKERS as i32).collect();
            consumer
                .assign(&in_topic_fetch, partitions)
                .await
                .expect("Failed to assign partitions");

            let seek_map: HashMap<(String, i32), i64> = (0..NUM_WORKERS as i32)
                .map(|p| ((in_topic_fetch.clone(), p), 0i64))
                .collect();
            consumer
                .seek_many(&seek_map)
                .await
                .expect("Failed to seek partitions");

            let mut consecutive_empty = 0u32;
            let mut records_buf: Vec<krafka::consumer::ConsumerRecord> = Vec::new();

            loop {
                if STOP_FLAG.load(Ordering::Relaxed) {
                    break;
                }

                let polled = match consumer.poll(Duration::from_millis(10)).await {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                if polled.is_empty() {
                    consecutive_empty += 1;
                    if consecutive_empty >= 3 {
                        break;
                    }
                    continue;
                }
                consecutive_empty = 0;

                records_buf.extend(polled);

                // Drain records_buf into CHUNK_SIZE sub-batches.
                // drain() moves ownership — no clone of individual records.
                while records_buf.len() >= CHUNK_SIZE {
                    let chunk: Vec<_> = records_buf.drain(..CHUNK_SIZE).collect();
                    if tx.send(chunk).await.is_err() {
                        return;
                    }
                }
            }

            // Flush any partial chunk remaining after the loop ends.
            if !records_buf.is_empty() {
                let _ = tx.send(records_buf).await;
            }
            // tx drops here, closing the channel.
        });
    });

    fetch_handle.join().expect("Fetch task panicked");
    for h in process_handles {
        h.join().expect("Process task panicked");
    }

    reporter_stop.store(true, Ordering::Relaxed);
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

        let total = PROCESSED_COUNTER.load(Ordering::Relaxed);
        println!("\n🏁 BENCHMARK RESULT (KRAFKA2) 🏁");
        println!("Total Records:   {}", total);
        println!("Processing Time: {:.2}s", duration_secs);
        println!("Throughput:      {:.0} msg/sec", total as f64 / duration_secs);
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
        assert_eq!(roundtrip(i64::MAX), i64::MAX);
    }

    #[test]
    fn test_negative() {
        assert_eq!(roundtrip(-1), -1);
        assert_eq!(roundtrip(-1000), -1000);
        assert_eq!(roundtrip(i64::MIN), i64::MIN);
    }
}
