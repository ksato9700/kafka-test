# stream-test-krafka Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a high-performance Kafka streaming benchmark in `stream-test-krafka/` using the pure-Rust krafka crate (no C deps), measuring 50M-record throughput.

**Architecture:** Three binaries share a crate — `processor` (benchmark), `producer` (dev utility), `consumer` (dev utility). The processor uses `std::thread` + per-thread `tokio::runtime::Runtime` (one per worker thread) for fairest comparison with `stream-test-rust`. A static `AtomicU64` pair tracks the first-message timestamp and processed count.

**Tech Stack:** krafka 0.7, tokio 1 (rt + rt-multi-thread only), rand 0.8 (SmallRng), log 0.4, env_logger 0.11.

---

## File Map

| Path | Purpose |
|---|---|
| `stream-test-krafka/Cargo.toml` | Crate manifest, 3 bin targets, release profile |
| `stream-test-krafka/.gitignore` | Ignore `/target` |
| `stream-test-krafka/.dockerignore` | Ignore `target/` |
| `stream-test-krafka/src/bin/processor.rs` | Benchmark binary: load phase + worker threads + reporter |
| `stream-test-krafka/src/bin/producer.rs` | Utility: continuous producer to `integer-list-input` |
| `stream-test-krafka/src/bin/consumer.rs` | Utility: print sums from `integer-sum-output` |
| `stream-test-krafka/Makefile` | Build + run targets |
| `stream-test-krafka/Dockerfile` | Multi-stage Alpine build |

---

## Task 1: Scaffold the crate

**Files:**
- Create: `stream-test-krafka/Cargo.toml`
- Create: `stream-test-krafka/.gitignore`
- Create: `stream-test-krafka/.dockerignore`
- Create: `stream-test-krafka/src/bin/processor.rs` (stub)
- Create: `stream-test-krafka/src/bin/producer.rs` (stub)
- Create: `stream-test-krafka/src/bin/consumer.rs` (stub)

- [ ] **Step 1: Create the directory structure**

```bash
mkdir -p stream-test-krafka/src/bin
```

- [ ] **Step 2: Write Cargo.toml**

```toml
[package]
name = "stream-test-krafka"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "processor"
path = "src/bin/processor.rs"

[[bin]]
name = "producer"
path = "src/bin/producer.rs"

[[bin]]
name = "consumer"
path = "src/bin/consumer.rs"

[dependencies]
krafka = "0.7"
tokio = { version = "1", features = ["rt", "rt-multi-thread"] }
rand = { version = "0.8", features = ["small_rng"] }
log = "0.4"
env_logger = "0.11"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
```

- [ ] **Step 3: Write .gitignore**

```
/target
```

- [ ] **Step 4: Write .dockerignore**

```
target/
```

- [ ] **Step 5: Create stub binaries so the crate compiles**

`stream-test-krafka/src/bin/processor.rs`:
```rust
fn main() {}
```

`stream-test-krafka/src/bin/producer.rs`:
```rust
fn main() {}
```

`stream-test-krafka/src/bin/consumer.rs`:
```rust
fn main() {}
```

- [ ] **Step 6: Verify the crate compiles**

Run from `stream-test-krafka/`:
```bash
cargo build
```
Expected: compiles without error.

- [ ] **Step 7: Commit**

```bash
git add stream-test-krafka/
git commit -m "feat(stream-test-krafka): scaffold crate with Cargo.toml and stub binaries"
```

---

## Task 2: Zig-Zag LEB128 helpers + tests in processor.rs

**Files:**
- Modify: `stream-test-krafka/src/bin/processor.rs`

The helpers appear in all three binaries. We start with processor.rs and verify correctness with tests before using them in the benchmark logic.

- [ ] **Step 1: Write the failing tests**

Replace `stream-test-krafka/src/bin/processor.rs` with:

```rust
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

fn main() {}

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
    }

    #[test]
    fn test_negative() {
        assert_eq!(roundtrip(-1), -1);
        assert_eq!(roundtrip(-64), -64);
        assert_eq!(roundtrip(-1000), -1000);
    }

    #[test]
    fn test_multi_value_sequence() {
        let mut buf = Vec::new();
        write_varint(&mut buf, 3);   // count
        write_varint(&mut buf, 10);
        write_varint(&mut buf, 20);
        write_varint(&mut buf, 30);
        write_varint(&mut buf, 0);   // terminator

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
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cargo test --bin processor
```
Expected: 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add stream-test-krafka/src/bin/processor.rs
git commit -m "feat(stream-test-krafka): add zig-zag LEB128 helpers with passing tests"
```

---

## Task 3: processor.rs — Phase 1 (load)

**Files:**
- Modify: `stream-test-krafka/src/bin/processor.rs`

Phase 1 runs only when `BENCHMARK=1` is set. It produces 50M records to `integer-list-input-benchmark` using a single-threaded krafka producer inside a new `tokio::Runtime`.

- [ ] **Step 1: Implement Phase 1**

Replace `stream-test-krafka/src/bin/processor.rs` with:

```rust
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
    }

    #[test]
    fn test_negative() {
        assert_eq!(roundtrip(-1), -1);
        assert_eq!(roundtrip(-64), -64);
        assert_eq!(roundtrip(-1000), -1000);
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
```

- [ ] **Step 2: Verify tests still pass and it compiles**

```bash
cargo test --bin processor && cargo build --bin processor
```
Expected: 4 tests pass, binary builds.

- [ ] **Step 3: Commit**

```bash
git add stream-test-krafka/src/bin/processor.rs
git commit -m "feat(stream-test-krafka): implement processor Phase 1 load"
```

---

## Task 4: processor.rs — Phase 2 (workers, reporter, result)

**Files:**
- Modify: `stream-test-krafka/src/bin/processor.rs`

Phase 2 spawns `NUM_WORKERS` OS threads. Each gets its own `tokio::Runtime`, `Consumer`, and `Producer`. A separate reporter thread prints progress every 1s. Main thread joins all workers, then prints the benchmark result.

- [ ] **Step 1: Implement Phase 2 — replace the full processor.rs**

```rust
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
    }

    #[test]
    fn test_negative() {
        assert_eq!(roundtrip(-1), -1);
        assert_eq!(roundtrip(-64), -64);
        assert_eq!(roundtrip(-1000), -1000);
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
```

- [ ] **Step 2: Run tests and build**

```bash
cargo test --bin processor && cargo build --bin processor
```
Expected: 4 tests pass, binary builds.

- [ ] **Step 3: Commit**

```bash
git add stream-test-krafka/src/bin/processor.rs
git commit -m "feat(stream-test-krafka): implement processor Phase 2 workers and reporter"
```

---

## Task 5: producer.rs

**Files:**
- Modify: `stream-test-krafka/src/bin/producer.rs`

Infinite loop producing Zig-Zag LEB128 records to `integer-list-input`. Logs a count every 10,000 records. Terminated by SIGINT (process kill — no graceful flush, as the `signal` tokio feature is not included).

- [ ] **Step 1: Write producer.rs**

```rust
use krafka::producer::Producer;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::env;

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
    env_logger::init();
    let bootstrap = env::var("BOOTSTRAP").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = "integer-list-input";

    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(async {
        let producer = Producer::builder()
            .bootstrap_servers(&bootstrap)
            .build()
            .await
            .expect("Failed to create producer");

        let mut rng = SmallRng::from_entropy();
        let mut buf = Vec::with_capacity(64);
        let mut count: u64 = 0;

        println!("🚀 Producer running. Sending to '{}'...", topic);

        loop {
            buf.clear();
            let n = rng.gen_range(2i64..6);
            write_varint(&mut buf, n);
            for _ in 0..n {
                write_varint(&mut buf, rng.gen_range(0i64..100));
            }
            write_varint(&mut buf, 0);

            let _ = producer.send(topic, None, &buf).await;
            count += 1;

            if count % 10_000 == 0 {
                println!("📤 Sent {} records", count);
            }
        }
    });
}
```

- [ ] **Step 2: Build**

```bash
cargo build --bin producer
```
Expected: compiles without error.

- [ ] **Step 3: Commit**

```bash
git add stream-test-krafka/src/bin/producer.rs
git commit -m "feat(stream-test-krafka): add producer utility binary"
```

---

## Task 6: consumer.rs

**Files:**
- Modify: `stream-test-krafka/src/bin/consumer.rs`

Subscribes to `integer-sum-output`, decodes each sum, prints `Sum: {value}`. Exits on SIGINT.

- [ ] **Step 1: Write consumer.rs**

```rust
use krafka::consumer::{AutoOffsetReset, Consumer};
use std::env;
use std::time::Duration;

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

fn main() {
    env_logger::init();
    let bootstrap = env::var("BOOTSTRAP").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = "integer-sum-output";

    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(async {
        let consumer = Consumer::builder()
            .bootstrap_servers(&bootstrap)
            .group_id("stream-test-krafka-consumer")
            .auto_offset_reset(AutoOffsetReset::Earliest)
            .enable_auto_commit(true)
            .build()
            .await
            .expect("Failed to create consumer");

        consumer
            .subscribe(&[topic])
            .await
            .expect("Failed to subscribe");

        println!("📥 Consumer listening on '{}'...", topic);

        loop {
            match consumer.poll(Duration::from_millis(100)).await {
                Ok(records) => {
                    for record in records {
                        if let Some(ref value_bytes) = record.value {
                            let mut payload: &[u8] = value_bytes.as_ref();
                            let sum = read_varint(&mut payload);
                            println!("Sum: {}", sum);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Poll error: {:?}", e);
                }
            }
        }
    });
}
```

- [ ] **Step 2: Build all three binaries**

```bash
cargo build
```
Expected: all three binaries compile without error.

- [ ] **Step 3: Commit**

```bash
git add stream-test-krafka/src/bin/consumer.rs
git commit -m "feat(stream-test-krafka): add consumer utility binary"
```

---

## Task 7: Makefile + Dockerfile

**Files:**
- Create: `stream-test-krafka/Makefile`
- Create: `stream-test-krafka/Dockerfile`

- [ ] **Step 1: Write Makefile**

```makefile
.PHONY: build run-benchmark run-stream run-producer run-consumer docker-build docker-run

BOOTSTRAP ?= 127.0.0.1:9094

build:
	cargo build --release

run-benchmark:
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-list-input-benchmark --partitions 8 --replication-factor 1
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-sum-output-benchmark --partitions 8 --replication-factor 1
	BENCHMARK=1 NUM_WORKERS=8 cargo run --release --bin processor

run-stream:
	cargo run --release --bin processor

run-producer:
	cargo run --release --bin producer

run-consumer:
	cargo run --release --bin consumer

docker-build:
	cd .. && docker build -f stream-test-krafka/Dockerfile -t stream-test-krafka .

docker-run:
	docker run --rm --network kafka-net \
	  -e BOOTSTRAP=kafka-broker:9092 \
	  stream-test-krafka
```

- [ ] **Step 2: Write Dockerfile**

```dockerfile
FROM rust:1-alpine3.23 AS builder
WORKDIR /usr/src/app
RUN apk add --no-cache musl-dev
COPY stream-test-krafka .
RUN cargo build --release

FROM alpine:3.23
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/processor /usr/local/bin/processor
COPY --from=builder /usr/src/app/target/release/producer /usr/local/bin/producer
COPY --from=builder /usr/src/app/target/release/consumer /usr/local/bin/consumer
CMD ["processor"]
```

- [ ] **Step 3: Commit**

```bash
git add stream-test-krafka/Makefile stream-test-krafka/Dockerfile
git commit -m "feat(stream-test-krafka): add Makefile and Dockerfile"
```

---

## Task 8: Release build verification

**Files:** (no changes)

Verify the release binary produces the correct artifact paths and the project is fully usable.

- [ ] **Step 1: Run release build**

```bash
cd stream-test-krafka && cargo build --release
```
Expected: compiles without warnings/errors.

- [ ] **Step 2: Verify all three binaries exist**

```bash
ls -lh target/release/processor target/release/producer target/release/consumer
```
Expected: three files, each > 1MB (statically linked release binaries).

- [ ] **Step 3: Run the tests one final time**

```bash
cargo test --bin processor
```
Expected: 4 tests pass.

- [ ] **Step 4: Commit (if any fixups were needed)**

If no changes were needed, skip this step. Otherwise:
```bash
git add -p
git commit -m "fix(stream-test-krafka): <describe fixup>"
```
