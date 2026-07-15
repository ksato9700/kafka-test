# Hardening Plan: Krafka Concurrency Safety, 0.12 Upgrade, & Durable Benchmarking

This document details the step-by-step implementation plan to upgrade the [krafka](file:///Users/ksato/git/kafka-test/stream-test-krafka/Cargo.toml) client dependency to the latest `0.12` version, and resolve the latent concurrency safety hazard in the krafka producer, as described in [krafka-idempotent-producer-concurrency-safety.md](file:///Users/ksato/git/kafka-test/docs/krafka-idempotent-producer-concurrency-safety.md).

---

## 1. Context and Objective

An idempotent Kafka producer requires that all batches sent to a given partition follow a strictly increasing sequence number. In the pure-Rust `krafka` client, sequence numbers are allocated atomically when the send future is first polled inside the runtime.

Currently, the Phase 1 loading stage in [processor.rs](file:///Users/ksato/git/kafka-test/stream-test-krafka/src/bin/processor.rs#L133-L159) uses `tokio::task::JoinSet::spawn()` to run concurrent sends. Under load, Tokio schedules these spawned tasks out of order, leading to out-of-order sequence number allocations if idempotency is enabled. This results in the broker throwing an `OutOfOrderSequenceNumber` error and aborting the stream. 

While the benchmark defaults to `idempotent(false)` and `Acks::None` today (making it safe from this error), this setup presents a high-risk latent bug if anyone attempts to configure the benchmark for production durability levels.

The objectives of this plan are:
1. **Upgrade Krafka Client**: Upgrade both krafka projects from `0.11` to `0.12` to leverage protocol improvements, performance enhancements, and the latest client features.
2. **Eliminate the Concurrency Race**: Migrate Phase 1 from `JoinSet::spawn()` to a single-task [FuturesUnordered](https://docs.rs/futures/latest/futures/stream/struct.FuturesUnordered.html) queue.
3. **Introduce Durable Benchmark Mode**: Allow enabling `idempotent(true)` and `Acks::All` dynamically via a `DURABLE=1` environment variable.
4. **Add Durable Make Target**: Add `make run-benchmark-durable` to run the benchmark with full durability.
5. **Document Concurrency Hazards**: Document this sequencing constraint in both the source code comments and [stream-sum-krafka-design.md](file:///Users/ksato/git/kafka-test/docs/stream-sum-krafka-design.md).

---

## 2. Detailed Task List

### Task 2.1: Upgrade `krafka` dependency to `0.12`
We will update [kafka-test-krafka/Cargo.toml](file:///Users/ksato/git/kafka-test/kafka-test-krafka/Cargo.toml) and [stream-test-krafka/Cargo.toml](file:///Users/ksato/git/kafka-test/stream-test-krafka/Cargo.toml):
- Change `krafka = "0.11"` to `krafka = "0.12"`.
- Run `cargo check` and fix any compilation errors resulting from the upgrade.
- *Compatibility assessment:* Neither project utilizes custom DLQs, Interceptors, Headers, or `ConsumerRecord::timestamp` unwrapping, so the upgrade is expected to be a drop-in replacement with no code changes required for compatibility. The minimum broker version requirement is now Kafka 3.9+, which is satisfied by our current Docker development broker (version 4.2.0).

### Task 2.2: Update `stream-test-krafka` Processor Configuration and Logic
We will modify [processor.rs](file:///Users/ksato/git/kafka-test/stream-test-krafka/src/bin/processor.rs):
- Retrieve `DURABLE` from the environment. Print whether durable mode (idempotent = true, acks = all) or best-effort mode (idempotent = false, acks = none) is active.
- Refactor the Phase 1 loading loop to use `FuturesUnordered` instead of `JoinSet`.
- Pass `acks` and `idempotent` dynamically based on the `DURABLE` flag in both the Phase 1 producer builder and the Phase 2 worker producer builders.
- Add code comments explaining the sequencing constraints to prevent future regressions.

### Task 2.3: Add Makefile Targets
We will modify [Makefile](file:///Users/ksato/git/kafka-test/stream-test-krafka/Makefile):
- Add a new target `run-benchmark-durable` which invokes:
  ```makefile
  BENCHMARK=1 DURABLE=1 NUM_WORKERS=8 cargo run --release --bin processor
  ```

### Task 2.4: Update Krafka Stream Sum Design Documentation
We will update [stream-sum-krafka-design.md](file:///Users/ksato/git/kafka-test/docs/stream-sum-krafka-design.md):
- Clarify the design of Phase 1 load concurrency in §3.
- Update the tech stack and comparison sections to reflect `krafka` v0.12.
- Document the sequencing constraint under §4 ("Key tuning lessons") as lesson 9.

---

## 3. Concrete Code Changes (Draft Diffs)

### 3.1 Changes to `kafka-test-krafka/Cargo.toml`
```diff
--- a/kafka-test-krafka/Cargo.toml
+++ b/kafka-test-krafka/Cargo.toml
@@ -7,3 +7,3 @@
 apache-avro = "0.21"
-krafka = "0.11"
+krafka = "0.12"
 tokio = { version = "1", features = ["full"] }
```

### 3.2 Changes to `stream-test-krafka/Cargo.toml`
```diff
--- a/stream-test-krafka/Cargo.toml
+++ b/stream-test-krafka/Cargo.toml
@@ -18,3 +18,3 @@
 [dependencies]
-krafka = "0.11"
+krafka = "0.12"
 tokio = { version = "1", features = ["rt", "rt-multi-thread", "time"] }
```

### 3.3 Changes to `stream-test-krafka/src/bin/processor.rs`

```diff
--- a/stream-test-krafka/src/bin/processor.rs
+++ b/stream-test-krafka/src/bin/processor.rs
@@ -53,8 +53,17 @@
         .parse()
         .expect("NUM_WORKERS must be a number");
 
+    let is_durable = env::var("DURABLE").is_ok();
+    if is_durable {
+        println!("🔒 Durable mode enabled (idempotency=true, acks=all)");
+    } else {
+        println!("⚡ Best-effort mode enabled (idempotency=false, acks=none)");
+    }
+
+    let acks = if is_durable { Acks::All } else { Acks::None };
+    let idempotent = is_durable;
+
     if env::var("BENCHMARK").is_ok() && env::var("SKIP_LOAD").is_err() {
         println!("🚀 Phase 1: Loading {} records (krafka)...", TOTAL_RECORDS);
         let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
         rt.block_on(async {
@@ -118,18 +127,24 @@
 
             let producer = Arc::new(
                 Producer::builder()
                     .bootstrap_servers(&bootstrap)
-                    .acks(Acks::None)
-                    .idempotent(false)
+                    .acks(acks)
+                    .idempotent(idempotent)
                     .linger(Duration::from_millis(5))
                     .batch_size(64 * 1024)
                     .build()
                     .await
                     .expect("Failed to create producer"),
             );
 
             let mut rng = SmallRng::from_entropy();
             let mut buf = Vec::with_capacity(64);
-            let mut join_set: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
+
+            // NOTE: We must use a single-task FuturesUnordered queue rather than tokio::task::JoinSet
+            // to guarantee that the futures are pushed and polled in order. This is critical for
+            // idempotent producers (idempotency=true), where sequence numbers are allocated
+            // atomically on the first poll. Out-of-order polling causes broker sequence errors.
+            let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
             // Keep up to 10k sends in flight simultaneously (fire-and-forget style)
             const WINDOW: usize = 10_000;
 
@@ -144,19 +159,19 @@
 
                 let p = Arc::clone(&producer);
                 let data = buf.clone();
-                join_set.spawn(async move {
+                in_flight.push(async move {
                     let _ = p.send("integer-list-input-benchmark", None, &data).await;
                 });
 
                 // Drain oldest completed send when window is full
-                if join_set.len() >= WINDOW {
-                    join_set.join_next().await;
+                if in_flight.len() >= WINDOW {
+                    in_flight.next().await;
                 }
             }
 
             // Wait for all in-flight sends to complete
-            while join_set.join_next().await.is_some() {}
+            while in_flight.next().await.is_some() {}
 
             println!("Flushing producer...");
             producer.flush().await.expect("Failed to flush producer");
@@ -207,11 +222,11 @@
         let handle = std::thread::spawn(move || {
             let rt = tokio::runtime::Runtime::new().expect("Failed to create worker runtime");
             rt.block_on(async move {
                 let producer = Producer::builder()
                     .bootstrap_servers(&bootstrap)
-                    .acks(Acks::None)
-                    .idempotent(false)
+                    .acks(acks)
+                    .idempotent(idempotent)
                     .linger(Duration::from_millis(5))
                     .batch_size(64 * 1024)
                     .build()
                     .await
```

### 3.4 Changes to `stream-test-krafka/Makefile`

```diff
--- a/stream-test-krafka/Makefile
+++ b/stream-test-krafka/Makefile
@@ -14,6 +14,15 @@
 	  --topic integer-sum-output-benchmark --partitions 8 --replication-factor 1
 	BENCHMARK=1 NUM_WORKERS=8 cargo run --release --bin processor
 
+run-benchmark-durable:
+	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
+	  --bootstrap-server localhost:9092 --create --if-not-exists \
+	  --topic integer-list-input-benchmark --partitions 8 --replication-factor 1
+	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
+	  --bootstrap-server localhost:9092 --create --if-not-exists \
+	  --topic integer-sum-output-benchmark --partitions 8 --replication-factor 1
+	BENCHMARK=1 DURABLE=1 NUM_WORKERS=8 cargo run --release --bin processor
+
 run-stream:
 	cargo run --release --bin processor
```

### 3.5 Changes to `docs/stream-sum-krafka-design.md`

```diff
--- a/docs/stream-sum-krafka-design.md
+++ b/docs/stream-sum-krafka-design.md
@@ -5,3 +5,3 @@
 Implement `streaming-performance-spec.md` in Rust using the **krafka** crate (v0.8) —
-a pure-Rust, async-native Kafka client — and benchmark it against `stream-test-rust`
+a pure-Rust, async-native Kafka client (upgraded to v0.12) — and benchmark it against `stream-test-rust`
 (rdkafka / librdkafka). The goal is to validate krafka's throughput ceiling and document
@@ -13,3 +13,3 @@
 - **Language**: Rust (Edition 2021, MSRV 1.88)
-- **Kafka Library**: `krafka = "0.8"` (published crate)
+- **Kafka Library**: `krafka = "0.12"` (upgraded from v0.8 / v0.11)
 - **Async Runtime**: `tokio` (multi-thread, one runtime per worker OS thread)
@@ -36,8 +36,9 @@
 
-A `JoinSet` window of 10,000 concurrent tasks drives a single `Arc<Producer>` with
-`Acks::None` (resolves at TCP layer, no broker RTT). The encode loop submits tasks
-and drains completed ones when the window fills, then awaits full drain and calls
-`flush()` before proceeding to Phase 2.
+A single-task `FuturesUnordered` pool of 10,000 concurrent futures drives a single 
+`Arc<Producer>`. Concurrency is managed inside a single thread without independent task
+spawning, guaranteeing that the sends are initiated and polled in order. This structural 
+alignment prevents sequence number drift when running with `idempotent(true)`. The loop
+submits futures and polls them to make progress when the window is full.
 
@@ -211,4 +212,12 @@
 
+9. **`JoinSet::spawn()` is unsafe with `idempotent(true)`.** Sequence-number
+   allocation happens atomically inside `send()`, but tokio doesn't guarantee
+   spawned tasks run in spawn order — concurrently spawned sends to the same
+   partition can have their sequence numbers allocated out of order, and the broker
+   rejects the result with `OutOfOrderSequenceNumber`. Use a single-task
+   `FuturesUnordered` instead (as Phase 2 already does) any time durability beyond
+   `idempotent(false)` is required — and keep `send()`/`send_record()` as that
+   future's only await point; an async encode/registry step ahead of it reintroduces
+   the same race through a different door.
```

---

## 4. Design Completeness Checklist (Self-Evaluation)

Before finalizing this plan, we run through the completeness checks to ensure no implicit knowledge is left unstated:

### Check 1 — Known Risks and Accepted Trade-offs
*   **Risk: Memory Pressure from Large Window Size**: 
    We keep `WINDOW = 10,000` concurrent sends in flight. Because `FuturesUnordered` drives all these futures in a single task, they are represented as memory allocations. At 10k messages, this consumes negligible memory (~few megabytes), which is fully acceptable given the performance requirement.
*   **Risk: Single-threaded FuturesUnordered Throughput**:
    Driving 10,000 futures in a single task using `FuturesUnordered` instead of spawning them as separate tasks in `JoinSet` eliminates the OS/Tokio scheduling overhead. We accept the risk that single-threaded execution could limit CPU utilization on multi-core systems, but previous testing has shown that the performance is actually higher due to the absence of `tokio::spawn` scheduling overhead.

### Check 2 — Data Contract & Schema Constraints
*   **Contract Baseline**: 
    The serialization format is a raw, custom Zig-Zag binary encoding (LEB128). This contract is upstream and external. We do not introduce any async await points in serialization (e.g. Schema Registry lookup), which guarantees that the payload is fully prepared synchronously before pushing to `in_flight`. This satisfies the strict sequencing requirement.

### Check 3 — Operational Infrastructure & Configuration
*   **Topic Properties**: 
    Runs against the local single-broker Kafka deployment defined in [run-kafka.sh](file:///Users/ksato/git/kafka-test/run-kafka.sh) (`127.0.0.1:9094`).
*   **Replication Constraints**: 
    Because replication factor is 1 (`KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1`), `Acks::All` is effectively equivalent to `Acks::Leader`. However, configuring `acks = All` forces the client to negotiate the full protocol pathway for durability, making it representative of a multi-node cluster benchmark.
*   **Broker Version requirement**:
    `krafka` v0.12 raises the MSRV (Minimum Supported Rust Version) to 1.88 and minimum broker version to Kafka 3.9+. Since our Docker container runs Kafka 4.2.0, the infrastructure configuration is fully compatible.

### Check 4 — Alternatives Considered and Rejected
*   **Alternative A: Introduce a Semaphore + JoinSet**:
    Instead of migrating to `FuturesUnordered`, we could use a tokio Semaphore to limit concurrency inside `JoinSet`. *Rejected* because task scheduling order is still random, meaning the sequence numbers would still be allocated out-of-order under high concurrency.
*   **Alternative B: Sequence Number Locking in Producer**:
    We could serialize the sequence number allocation by wrapping the producer in a custom sequential wrapper. *Rejected* because it introduces application-level locking overhead and duplicates logic that `krafka` already handles internally.

### Check 5 — Scope Boundaries
*   **Out of Scope**: 
    We do not address the architectural 5× performance gap between `krafka` and `rdkafka` (caused by sequential fetch/process and the absence of consumer background prefetch). Pipelining consumer fetches requires internal library changes in `krafka` and is out of scope.
*   **In Scope**: 
    Correcting the concurrent producer implementation pattern in this repository to prevent data loss or crashes under `idempotent(true)`, and upgrading `krafka` to `0.12`.

### Check 6 — Failure Scenarios Walked End-to-End
*   **Scenario: Out-of-Order Sequence Number Error**:
    1. Loop spawns Task A (sequence number not yet allocated) then Task B.
    2. Tokio scheduler schedules Task B first; `krafka` assigns sequence number `0` to Task B.
    3. Task A is scheduled second; `krafka` assigns sequence number `1` to Task A.
    4. Task A is written to the TCP socket and arrives at the broker first.
    5. The broker expects sequence number `0` first but receives `1`.
    6. The broker rejects the request with an `OutOfOrderSequenceNumber` protocol error, crashing the producer.
    *With this fix (FuturesUnordered)*: Task A is pushed and polled first, guaranteeing sequence number `0` is assigned and sent before Task B is polled.

---

## 5. Verification Plan

We will verify this plan in three steps:

1.  **Dependency Upgrade & Compilation**:
    - Apply the version changes in both `Cargo.toml` files.
    - Run:
      ```bash
      cargo check
      ```
    - Resolve any compiler warnings or errors.
2.  **Durable Verification (No Concurrency Crash)**:
    - Run the benchmark with durable mode enabled:
      ```bash
      cd stream-test-krafka
      make run-benchmark-durable
      ```
    - Verify that 50,000,000 records are produced and processed without any broker crashes or `OutOfOrderSequenceNumber` errors.
3.  **Performance Regression Check**:
    - Run the default benchmark mode:
      ```bash
      make run-benchmark
      ```
    - Check that the throughput (~680K msg/sec) remains stable or improves compared to the old `JoinSet` implementation.
4.  **Benchmark Documentation Update**:
    - Gather performance results for `make run-benchmark` and `make run-benchmark-durable` and report them side-by-side in [BENCHMARK_SUMMARY.md](file:///Users/ksato/git/kafka-test/BENCHMARK_SUMMARY.md).
