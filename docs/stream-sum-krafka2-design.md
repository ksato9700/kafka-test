# Design Document: stream-test-krafka2

## Status: CONCLUDED — Target not achieved; pivoting to rdkafka

---

## 1. Objective

Close the throughput gap between krafka v0.8 and rdkafka by restructuring the
consumer/producer pipeline. `stream-test-krafka` (v0.8) achieved ~682K msg/sec — about
5× behind rdkafka's ~3.5M msg/sec. This implementation targeted ≥2M msg/sec by
eliminating the two structural bottlenecks identified in that baseline:

1. **Sequential fetch → process cycle** — fetch and process are serialised per worker;
   the worker idles during the broker round-trip.
2. **8 independent FetchRequests** — each worker issues its own FetchRequest; the broker
   handles 8 instead of 1.

---

## 2. Root Cause Recap

rdkafka's loop:
```rust
// Both calls are in-memory operations; all network I/O is in background C threads.
if let Some(Ok(m)) = consumer.poll(10ms) { process(m); producer.send(m); }
```

krafka v0.8 loop:
```
[fetch RTT ~10ms] → [process ~100ms] → [fetch RTT] → ...
```

The worker is idle for the entire fetch RTT on every cycle. With 50ms `poll()` timeout
and 8 workers each issuing separate FetchRequests, the broker is handling 8 × (50K
records × ~10 bytes) = ~4MB of fetch traffic per cycle, fragmented across 8 connections.

---

## 3. Proposed Architecture (Implemented)

### Key changes vs stream-test-krafka

| | stream-test-krafka | stream-test-krafka2 |
|---|---|---|
| Consumers | 8 (one per worker) | 1 (all 8 partitions) |
| FetchRequests per cycle | 8 | 1 |
| Fetch model | Synchronous per worker | Dedicated fetch task, continuous loop |
| Process model | Same task as fetch | N parallel process tasks via channel |
| Producer per worker | Yes | Yes (one per process task) |

### Topology

```
                     ┌─────────────────────────────┐
                     │        fetch_task            │
                     │  consumer.poll() tight loop  │
                     │  (all 8 partitions assigned) │
                     │  splits into CHUNK_SIZE sub- │
                     │  batches before sending      │
                     └──────────┬──────────────────┘
                                │ Vec<ConsumerRecord> sub-batches (~5K records each)
                    ┌───────────▼────────────────────┐
                    │   tokio::sync::mpsc channel    │
                    │   capacity = NUM_WORKERS * 4   │
                    └──┬────┬────┬────┬────┬────┬───┘
                       │    │    │    │    │    │
                  ┌────▼┐ ┌─▼──┐ ...             ┌─▼──┐
                  │proc │ │proc│                  │proc│
                  │ 0   │ │ 1  │                  │ N-1│
                  │     │ │    │                  │    │
                  │prod │ │prod│                  │prod│
                  └─────┘ └────┘                  └────┘
```

---

## 4. Additional Tuning vs stream-test-krafka

| Setting | stream-test-krafka | stream-test-krafka2 | Rationale |
|---|---|---|---|
| `fetch_min_bytes` | 1 (default) | 1,000,000 | Don't respond until 1MB ready; forces large batches like rdkafka |
| `max_partition_fetch_bytes` | 10MB | 10MB | Unchanged |
| `max_poll_records` | 50,000 | 50,000 | Unchanged |
| `poll()` timeout | 50ms | 10ms | Fetch task loops continuously; short timeout reduces idle time |
| Producer `linger` | 5ms | 5ms | Unchanged |
| Producer `batch_size` | 64KB | 64KB | Unchanged |
| Producer `acks` | None | None | Fire-and-forget |

---

## 5. Implementation Iterations and Findings

### Iteration 1: Sequential `send_record().await` per record

**Throughput: ~1,300 msg/sec total**

The process task did a sequential `for record in records { producer.send_record().await }`.
With `linger=5ms`, each `.await` blocked for ~5ms — the "Async Sequential Trap":
the producer's accumulator waits for more records before flushing, but the task
cannot provide more records because it is blocked on `.await`. Throughput was
limited to `1/linger_ms ≈ 200 msg/sec` per worker.

Confirmed by micro-benchmark (`send_latency` binary):
- Sequential await: **168 msg/sec**
- `FuturesUnordered` (window=1000): **218K msg/sec**

### Iteration 2: `FuturesUnordered` with stop-the-world drain

**Throughput: ~130K msg/sec**

Replaced sequential loop with `FuturesUnordered`. Push all records in a sub-batch,
then `while sends.next().await.is_some() {}` before fetching next batch.

Improvement confirmed (1300 → 130K), but still far from target. Two hypotheses tested:

**Hypothesis A — single-thread executor starvation from blocking `rx.recv()`:**
Swapping to `new_multi_thread(worker_threads=2)` yielded **126K msg/sec** — effectively
identical. Hypothesis refuted. The OS thread is not the bottleneck.

**Hypothesis B — stop-the-world drain between sub-batches:**
`while sends.next().await` drains all 5K futures to zero before calling `rx.recv()`.
During drain the worker is not receiving new batches; during `rx.recv()` the executor
is not draining sends. Throughput profile showed consistent "stuttering": burst→drain→wait.

### Iteration 3: Pipelined `tokio::select!` with async `mpsc`

**Throughput: ~102K msg/sec — regression**

Switched from `crossbeam::bounded` to `tokio::sync::mpsc` (async recv) and introduced
`tokio::select!` to interleave batch receipt and send completion. Throughput dropped.

Root cause: sharing a single `mpsc::Receiver` across 8 workers required
`Arc<Mutex<Receiver>>`. All 8 workers contend on this mutex in the select! hot path,
serializing channel access. The serialization overhead exceeded the gain from
interleaving, producing a net regression.

### Observed pattern across all iterations

All three iterations showed the same "late-run acceleration":
- ~100–130K msg/sec for the first 80% of records
- ~500–600K msg/sec for the final 20%

The acceleration triggers precisely when the fetch task finishes (partitions drained)
and drops the channel sender. This is the definitive evidence that the bottleneck
lives in the **fetch task**, not the process tasks. The process tasks were consistently
starved: the fetch task was not delivering records fast enough to keep them busy.

---

## 6. Root Cause: Fetch Starvation

The process tasks were achieving the `send_latency` benchmark ceiling (~200K msg/sec
per worker in isolation) but the fetch task could only sustain ~130K msg/sec delivery.

`consumer.poll(10ms)` with `fetch_min_bytes=1MB` means: the broker waits up to 10ms
before responding even if < 1MB is ready. In our Lima/Docker environment, each poll
RTT is ~10ms, giving a maximum of **100 polls/sec**. Even with `max_poll_records=50K`,
the fetch task cannot exceed `100 polls/sec × 50K records = 5M records/sec` in theory
— but achieving that ceiling requires the broker to always have 1MB ready within the
10ms window, which is not guaranteed.

More fundamentally: krafka's `consumer.poll()` is **reactive** — it issues one
FetchRequest and waits for the response before issuing the next. librdkafka runs
background C threads that **proactively** pre-fetch and buffer records, so `poll()`
is a local memory operation. The RTT cost is paid once; krafka pays it every call.

---

## 7. Conclusion: Performance Ceiling of krafka v0.8

**Final throughput achieved: ~130K msg/sec**
**Target: ≥2M msg/sec**
**Gap: ~15×**

The bottleneck is not application-level logic but the current maturity of the
pure-Rust krafka implementation:

| Factor | krafka v0.8 | librdkafka |
|---|---|---|
| Background pre-fetch | No — reactive poll | Yes — C threads proactively buffer |
| FetchRequest model | One at a time, awaited | Pipelined, concurrent in background |
| Network I/O thread | Tokio executor (shared) | Dedicated native C threads |
| Lock-free internals | No — Tokio Mutex/Semaphore | Extensive lock-free ring buffers |
| Years of optimization | ~2 years | 10+ years |

krafka offers better deployment ergonomics (no C toolchain, no librdkafka DSO) and
is well-suited for moderate-throughput workloads. At 2M+ msg/sec, it cannot currently
match librdkafka's "mechanical sympathy."

**Decision: halt krafka2 development. Next step: implement the same fan-out
pipeline architecture using rdkafka.**

---

## 8. Lessons Learned

| Lesson | Detail |
|---|---|
| Async Sequential Trap | `send_record().await` in a loop is limited to `1/linger_ms` msg/sec per worker. Always use `FuturesUnordered` or equivalent for high-throughput send loops. |
| Stop-the-world drain | Draining `FuturesUnordered` to zero before fetching next batch recreates sequential stop-the-world behaviour between sub-batches. |
| Shared receiver contention | Sharing `mpsc::Receiver` via `Arc<Mutex>` across N threads serializes channel access. Use per-worker channels or MPMC (crossbeam) instead. |
| Bottleneck location | The "late-run acceleration" pattern definitively locates the bottleneck in the fetch path, not the process path. Always identify which stage is the constraint before optimizing. |
| Micro-benchmarks first | The `send_latency` binary confirmed the linger trap hypothesis in minutes, saving hours of full-benchmark iteration. |

---

## 9. Project Structure

```
stream-test-krafka2/
├── Cargo.toml          # krafka 0.8, futures, tokio, rand, tracing-subscriber
├── Makefile
├── Dockerfile
└── src/bin/
    ├── processor.rs    # Phase 1 load + Phase 2 fetch/process pipeline (final iteration)
    └── send_latency.rs # Micro-benchmark: sequential vs FuturesUnordered vs linger/mif variations
```
