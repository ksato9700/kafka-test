# Design Document: High-Performance Krafka Streaming Sum Processor

## 1. Objective

Implement `streaming-performance-spec.md` in Rust using the **krafka** crate (v0.12) —
a pure-Rust, async-native Kafka client — and benchmark it against `stream-test-rust`
(rdkafka / librdkafka). The goal is to validate krafka's throughput ceiling and document
the architectural differences that explain any performance gap.

## 2. Architecture & Technology Stack

- **Language**: Rust (Edition 2024, MSRV 1.88)
- **Kafka Library**: `krafka = "0.12"` (upgraded from v0.8 / v0.11)
- **Async Runtime**: `tokio` (multi-thread, one runtime per worker OS thread)
- **Serialization**: Manual Zig-Zag (LEB128) — no Avro library overhead
- **Concurrency Model**: Each worker runs on a dedicated OS thread with its own
  `tokio::runtime::Runtime`, mirroring rdkafka's per-thread model

## 3. Implementation

### Project Structure

```
stream-test-krafka/
├── Cargo.toml          # krafka 0.12, tokio, futures, rand, tracing-subscriber
├── Makefile            # run-benchmark, run-benchmark-durable, docker-build, docker-run-benchmark
├── Dockerfile          # Multi-stage: rust:1.88-alpine builder → alpine runtime
├── .dockerignore / .gitignore
└── src/bin/
    ├── processor.rs    # Entry point: Phase 1 load + Phase 2 worker spawn
    ├── producer.rs     # Standalone producer (non-benchmark mode)
    └── consumer.rs     # Standalone consumer (non-benchmark mode)
```

### Phase 1 — Load

A single-task `FuturesUnordered` pool of 10,000 concurrent futures drives a single 
`Arc<Producer>`. Concurrency is managed inside a single thread without independent task
spawning, guaranteeing that the sends are initiated and polled in order. This structural 
alignment prevents sequence number drift when running with `idempotent(true)`. The loop
submits futures and polls them to make progress when the window is full.

Phase 1 only runs when the `BENCHMARK` environment variable is set.

### Phase 2 — Process

Each of `NUM_WORKERS` workers runs on its own OS thread + Tokio runtime and owns:
- One `Consumer` (no group ID; `assign()` + explicit `seek(partition, 0)`, `max_poll_records=50_000`, `max_partition_fetch_bytes=4MB`)
- One `Producer` (`Acks::None`, `linger=5ms`, `batch_size=64KiB`)
- A bounded `mpsc::channel` (capacity 10,000) decoupling the consume loop from producer
  I/O. The bound is load-bearing, not incidental: it applies backpressure from the
  producer back to the consume loop when production falls behind, capping memory. An
  earlier, unbounded version of this channel caused an OOM crash under load (see §4).

Each worker is assigned exactly one partition via `consumer.assign()` — bypassing the
group coordinator entirely (no rebalances, no heartbeats, no `CoordinatorNotAvailable`
errors under concurrent startup). `seek(partition, 0)` is called explicitly after assign
to guarantee the fetch offset is resolved even if the `ListOffsets` RPC during
`apply_auto_offset_reset` is slow or races with metadata propagation.

The consume loop calls `consumer.poll(50ms)` in a tight loop. Each call issues one
`FetchRequest` to the broker with `max_wait_ms=50`. Records are decoded (Zig-Zag),
summed, and sent to the channel — `tx.send(buf).await` blocks once the channel is full,
which is the mechanism that throttles the consume loop when the producer falls behind. A
separate `produce_task` drains the channel via a `FuturesUnordered` pool of in-flight
sends (also capped at 10,000, gating the "accept new work" branch of its
`tokio::select!` once full) so the consume loop is never blocked on send I/O directly,
while total unacknowledged work is still bounded.

Termination: after 3 consecutive empty polls (partition drained), the consume loop
breaks. `tx` is dropped, signalling `produce_task` to drain its `FuturesUnordered` queue
and flush the remaining `local_counter` into `PROCESSED_COUNTER`.

In non-benchmark mode (no `BENCHMARK` env var), Phase 2 runs continuously against the
`integer-list-input` / `integer-sum-output` topics.

### Configuration (Environment Variables)

| Variable | Default | Description |
|---|---|---|
| `BOOTSTRAP` | `127.0.0.1:9094` | Broker address |
| `NUM_WORKERS` | `8` | Parallel worker threads |
| `BENCHMARK` | _(unset)_ | Set to any value to enable benchmark mode (Phase 1 + Phase 2 with fixed record count) |
| `SKIP_LOAD` | _(unset)_ | Set alongside `BENCHMARK` to skip Phase 1 (reuse existing topic data) |

Topic names are hardcoded:
- Benchmark mode: `integer-list-input-benchmark` → `integer-sum-output-benchmark`
- Normal mode: `integer-list-input` → `integer-sum-output`

## 4. Performance Findings

### Measured result (krafka v0.12)

| Metric | Value |
|---|---|
| Total records | 50,000,000 |
| Processing time | 74.88 s |
| Throughput | 667,741 msg/sec |
| Hardware | Apple M4, macOS, Kafka 4.2.0 (Docker / Lima) |
| Workers | 8 |

This is a **~3.6× improvement over krafka v0.7** (~185K msg/sec), in line with the v0.8
result previously measured here (~682K msg/sec) — the v0.8→v0.12 upgrade did not
regress or meaningfully change throughput.

**Note:** the first krafka 0.12 benchmark run crashed (`Killed: 9`) partway through
Phase 2 with an out-of-memory kill, caused by the `mpsc::unbounded_channel` +
unbounded `FuturesUnordered` described below having no backpressure — consumption
(batched, CPU-bound) outran production (network-bound) until the backlog exhausted
memory. Both are now bounded to a 10,000-item window (matching Phase 1's loader); the
measured result above reflects the fixed, bounded pipeline. See
`BENCHMARK_SUMMARY.md` ("Fixed: OOM crash...") for the root-cause writeup.

### What was tried and why it was slow (v0.7 era)

During the v0.7 implementation, several approaches were explored before arriving at the
current design:

| Attempt | Problem | Root cause |
|---|---|---|
| `producer.send(...).await?` in a loop | Phase 1 took 3+ minutes | Each await blocks until broker ACK — 50M sequential RTTs |
| `FuturesUnordered` of `send_record` | Still slow — window_drain_ms ≈ 72% of wall time | `FuturesUnordered` only makes progress when explicitly polled; futures stall between loop iterations |
| `tokio::spawn` per send + `Semaphore` | Phase 1 ~680k msg/sec, still ~5× slower than rdkafka | Task spawn overhead × 50M; rdkafka's C ring buffer costs ~50ns vs ~400ns per Tokio task |
| `Acks::None` + `JoinSet` window + channel decoupling (current) | Best result; still ~19× slower than rdkafka in Phase 2 | Consumer `poll()` structural bottleneck (see below) |

### API surprises in v0.8 (upgrade from v0.7)

Upgrading from v0.7 to v0.8 required several non-obvious fixes:

- **`batch_recv` takes a second `Duration` argument.** v0.7 used a single `max_records`
  argument; v0.8 separates the accumulation timeout: `batch_recv(max_records, timeout)`.
- **`BatchRecvOutcome` is `#[non_exhaustive]`.** Match arms must include a `Ok(_) => {}`
  wildcard even when all four named variants are handled.
- **`create_topics` / `delete_topics` require a `Duration` timeout argument** as the
  second parameter.
- **`describe_topics` takes `&[String]`, not `&[&str]`.**
- **`fetch_watermarks` takes 2 arguments** (topic + partition), not 3.
- **krafka uses `tracing`, not `log`.** `RUST_LOG=krafka=debug` with `env_logger` produces
  no output. Replace with `tracing_subscriber`.
- **`batch_recv` timeout doubles as `max_wait_ms`.** The same duration is passed both as
  the outer accumulation deadline and as the broker's `max_wait_ms`. With `100ms`, the
  broker's round-trip through Docker/Lima consumed the full deadline, returning `TimedOut`
  on every call despite records being available. The fix is to use `consumer.poll(50ms)`
  directly so the broker timeout and the application loop timeout are decoupled.
- **`assign()` + `auto_offset_reset` race.** `assign()` calls `apply_auto_offset_reset()`
  which issues a `ListOffsets` RPC. If this RPC is slow or races with metadata, the
  partition gets no tracked offset and is silently skipped on every subsequent fetch.
  Fix: call `consumer.seek(topic, partition, 0)` explicitly after `assign()`.

### The producer bottleneck: `send().await` is always ACK-gated

Unlike rdkafka's `ThreadedProducer.send()` (enqueues into a C lock-free ring buffer and
returns in nanoseconds), krafka's `send().await` always resolves after broker
acknowledgement (or TCP write with `Acks::None`). There is no synchronous enqueue path.
The async accumulator channel adds latency that cannot be eliminated at the application
level.

### The consumer bottleneck: sequential fetch → process cycle

Every call to `consumer.poll()` issues a `FetchRequest` to the broker and awaits the
response before returning. librdkafka continuously pre-fetches into a background C thread
buffer so application `poll()` calls read from memory and return in microseconds.
krafka v0.8 has no background prefetch loop; each `poll()` call pays one full network
round-trip (RTT) regardless of `max_poll_records`.

With 50K records per fetch, each cycle is approximately:
- Broker fetch RTT (~5–15ms through Docker/Lima)
- Record processing (~100ms for 50K records at ~2μs/record)
- Channel send (sub-millisecond)

This gives a per-worker ceiling of roughly 400K–600K msg/sec. With 8 workers summing to
~682K total, most of that ceiling is network-and-processing rather than CPU.

The **structural gap** versus rdkafka is that fetch and process are sequential in krafka
but pipelined (overlapped) in rdkafka:

```
rdkafka:  [prefetch running continuously in background]
app:               [pop+process] [pop+process] [pop+process] ...

krafka:   [fetch RTT] [process] [fetch RTT] [process] ...
```

Closing this gap would require a background-prefetch consumer in krafka — either a
streaming iterator that issues the next fetch while the application processes the current
batch, or an internal queue model. No such API exists in v0.8.

### Additional structural gap: 8 separate FetchRequests vs 1

Our design assigns each worker its own `Consumer` and its own partition, so 8 independent
`FetchRequest`s are sent to the broker per poll cycle — one per connection. rdkafka's
group consumer background thread sends a **single FetchRequest** containing all assigned
partitions, letting the broker handle one request instead of eight. At high message rates
this reduces broker scheduling overhead meaningfully.

### Key tuning lessons

1. **Use `consumer.poll(short_timeout)` rather than `batch_recv(n, short_timeout)`.** With
   `batch_recv`, the same duration caps both the broker `max_wait_ms` and the outer
   accumulation deadline. A short timeout causes every call to return `TimedOut` if the
   round-trip slightly exceeds it; `poll()` decouples these concerns.

2. **Call `seek(partition, 0)` explicitly after `assign()`.** Relying on
   `auto_offset_reset=Earliest` is fragile — the internal `ListOffsets` RPC can be slow or
   fail transiently, leaving the partition with no tracked offset and silently skipping all
   fetches.

3. **`Acks::None` is required for competitive producer throughput.** `Acks::Leader` or
   `Acks::All` adds a full broker RTT to every batch flush even with `FuturesUnordered`.

4. **Never `await` sends inside the consume loop.** Use a channel + dedicated produce
   task so the consume loop is never blocked on I/O.

5. **Per-worker OS thread + own `Runtime`.** Sharing one multi-thread Tokio pool across
   all workers causes work-stealing scheduler contention that measurably reduces throughput.

6. **`max_poll_records=50_000` and `max_partition_fetch_bytes=4MB`** are needed to
   amortise the per-poll RTT over as many records as possible.

7. **`FuturesUnordered` alone does not make sends concurrent** unless it is polled
   continuously (e.g., via `tokio::select!`). A naïve `while let Some(r) = futs.next().await`
   drain loop serialises rather than pipelines.

8. **Drain termination requires counting consecutive empty polls.** After a partition is
   exhausted, `poll()` returns empty indefinitely. The consume loop must break on N
   consecutive empty results and drop `tx` so the produce task can flush its
   `local_counter` remainder and exit cleanly.

9. **`JoinSet::spawn()` is unsafe with `idempotent(true)`.** Sequence-number
   allocation happens atomically inside `send()`, but tokio doesn't guarantee
   spawned tasks run in spawn order — concurrently spawned sends to the same
   partition can have their sequence numbers allocated out of order, and the broker
   rejects the result with `OutOfOrderSequenceNumber`. Use a single-task
   `FuturesUnordered` instead (as Phase 2 already does) any time durability beyond
   `idempotent(false)` is required — and keep `send()`/`send_record()` as that
   future's only await point; an async encode/registry step ahead of it reintroduces
   the same race through a different door.

10. **A channel that "decouples the consume loop from producer I/O" still needs a
    bound.** Removing the producer `await` from the consume loop's hot path (lesson 4)
    is not the same as removing backpressure entirely — if the channel and the
    downstream `FuturesUnordered` are both unbounded, the consumer (batched,
    CPU-bound) can outrun the producer (network-bound) indefinitely, and the backlog
    grows until the process is OOM-killed. Bound both to the same window size used
    elsewhere (10,000, matching Phase 1's loader) so the producer's real throughput
    caps how far ahead the consumer can get.

## 5. Comparison vs. `stream-test-rust` (rdkafka)

| Aspect | stream-test-krafka (v0.12) | stream-test-rust (rdkafka) |
|---|---|---|
| C dependency | None | Yes (librdkafka, cmake) |
| Cross-compilation | Pure Rust (`cargo build --target …`) | Requires C cross-compiler |
| Producer send model | Async, ACK-gated | Sync enqueue into C ring buffer |
| Consumer fetch model | Sync per-poll network fetch | Background C prefetch thread |
| Observed throughput (8 workers, local broker) | 667,741 msg/sec | ~3.5M msg/sec |
| Throughput gap | ~5× | N/A |
| Gap closable by app tuning? | Partially — pipelining fetch+process would help | N/A |

The remaining ~5× gap (down from ~19× with v0.7) is primarily the sequential fetch+process
cycle vs rdkafka's continuous background prefetch, plus the 8-separate-requests overhead.

## 6. Implementation Checklist

- [x] `processor.rs` — Phase 1 `FuturesUnordered` load + Phase 2 bounded-channel consume/produce loop, per-OS-thread runtime
- [x] `producer.rs` — standalone producer for normal (non-benchmark) mode
- [x] `consumer.rs` — standalone consumer for normal (non-benchmark) mode
- [x] Inline Zig-Zag encode/decode (`read_varint` / `write_varint`)
- [x] Progress reporter (1-second interval)
- [x] Final result log: `BENCHMARK RESULT (KRAFKA)`, total records, seconds, msg/sec
- [x] `Cargo.toml` — krafka 0.12 (published), tokio, futures, rand, tracing-subscriber
- [x] `Makefile` — `run-benchmark`, `docker-build`, `docker-run-benchmark`
- [x] `Dockerfile` — multi-stage Alpine build
- [x] `assign()` + explicit `seek(0)` per worker partition
- [x] `poll(50ms)` loop with consecutive-empty-poll termination
- [x] `SKIP_LOAD` env var to skip Phase 1 reload
