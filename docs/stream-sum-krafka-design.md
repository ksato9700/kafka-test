# Design Document: High-Performance Krafka Streaming Sum Processor

## 1. Objective

Implement `streaming-performance-spec.md` in Rust using the **krafka** crate (v0.7) —
a pure-Rust, async-native Kafka client — and benchmark it against `stream-test-rust`
(rdkafka / librdkafka). The goal is to validate krafka's throughput ceiling and document
the architectural differences that explain any performance gap.

## 2. Architecture & Technology Stack

- **Language**: Rust (Edition 2021, MSRV 1.88)
- **Kafka Library**: `krafka = "0.7"` (published crate)
- **Async Runtime**: `tokio` (multi-thread, one runtime per worker OS thread)
- **Serialization**: Manual Zig-Zag (LEB128) — no Avro library overhead
- **Concurrency Model**: Each worker runs on a dedicated OS thread with its own
  `tokio::runtime::Runtime`, mirroring rdkafka's per-thread model

## 3. Implementation

### Project Structure

```
stream-test-krafka/
├── Cargo.toml          # krafka 0.7, tokio, futures, rand, env_logger
├── Makefile            # run-benchmark, docker-build, docker-run-benchmark
├── Dockerfile          # Multi-stage: rust:1.88-alpine builder → alpine runtime
├── .dockerignore / .gitignore
└── src/bin/
    ├── processor.rs    # Entry point: Phase 1 load + Phase 2 worker spawn
    ├── producer.rs     # Standalone producer (non-benchmark mode)
    └── consumer.rs     # Standalone consumer (non-benchmark mode)
```

### Phase 1 — Load

A `JoinSet` window of 10,000 concurrent tasks drives a single `Arc<Producer>` with
`Acks::None` (resolves at TCP layer, no broker RTT). The encode loop submits tasks
and drains completed ones when the window fills, then awaits full drain and calls
`flush()` before proceeding to Phase 2.

Phase 1 only runs when the `BENCHMARK` environment variable is set.

### Phase 2 — Process

Each of `NUM_WORKERS` workers runs on its own OS thread + Tokio runtime and owns:
- One `Consumer` (`max_poll_records=50_000`, `max_partition_fetch_bytes=10MB`)
- One `Producer` (`Acks::None`, `linger=5ms`, `batch_size=64KiB`)
- An `mpsc::unbounded_channel` decoupling the consume loop from producer I/O

The consume loop polls records, decodes Zig-Zag, encodes the sum, and puts the result
on the channel. A separate `produce_task` drains the channel via `FuturesUnordered`
and `tokio::select!` so sends never block the consume loop.

In non-benchmark mode (no `BENCHMARK` env var), Phase 2 runs continuously against the
`integer-list-input` / `integer-sum-output` topics.

### Configuration (Environment Variables)

| Variable | Default | Description |
|---|---|---|
| `BOOTSTRAP` | `127.0.0.1:9094` | Broker address |
| `NUM_WORKERS` | `8` | Parallel worker threads |
| `BENCHMARK` | _(unset)_ | Set to any value to enable benchmark mode (Phase 1 + Phase 2 with fixed record count) |

Topic names are hardcoded:
- Benchmark mode: `integer-list-input-benchmark` → `integer-sum-output-benchmark`
- Normal mode: `integer-list-input` → `integer-sum-output`

## 4. Performance Findings

### What was tried and why it was slow

During implementation, several approaches were explored before arriving at the current
design. The table below summarises the progression:

| Attempt | Problem | Root cause |
|---|---|---|
| `producer.send(...).await?` in a loop | Phase 1 took 3+ minutes | Each await blocks until broker ACK — 50M sequential RTTs |
| `FuturesUnordered` of `send_record` | Still slow — window_drain_ms ≈ 72% of wall time | `FuturesUnordered` only makes progress when explicitly polled; futures stall between loop iterations |
| `tokio::spawn` per send + `Semaphore` | Phase 1 ~680k msg/sec, still ~5× slower than rdkafka | Task spawn overhead × 50M; rdkafka's C ring buffer costs ~50ns vs ~400ns per Tokio task |
| `Acks::None` + `JoinSet` window + channel decoupling (current) | Best result; still ~19× slower than rdkafka in Phase 2 | Consumer `poll()` structural bottleneck (see below) |

### The producer bottleneck: `send().await` is always ACK-gated

Unlike rdkafka's `ThreadedProducer.send()` (enqueues into a C lock-free ring buffer and
returns in nanoseconds), krafka's `send().await` always resolves after broker
acknowledgement (or TCP write with `Acks::None`). There is no synchronous enqueue path.
The async accumulator channel adds latency that cannot be eliminated at the application
level.

### The consumer bottleneck: `poll()` is synchronous per call

Every call to `consumer.poll()` issues a fresh `FetchRequest` to the broker and awaits
the response before returning. librdkafka continuously pre-fetches into a background C
thread buffer so `poll()` reads from memory and returns in microseconds. krafka v0.7 has
no background prefetch loop, so each poll call pays one full network RTT regardless of
`max_poll_records`. This is confirmed by reading `consumer/mod.rs`: `batch_fetch_from_broker`
is called synchronously inside every `poll()`.

At 8 workers on a local broker the measured throughput gap is approximately **19×**:
rdkafka ~3.5M msg/sec vs. krafka ~185k msg/sec (`Acks::None`).

### Key tuning lessons

1. **`Acks::None` is required for competitive producer throughput.** `Acks::Leader` or
   `Acks::All` adds a full broker RTT to every batch flush even with `FuturesUnordered`.

2. **Never `await` sends inside the consume loop.** Use a channel + dedicated produce
   task so the consume loop is never blocked on I/O.

3. **Per-worker OS thread + own `Runtime`.** Sharing one multi-thread Tokio pool across
   all workers causes work-stealing scheduler contention that measurably reduces throughput.

4. **`max_poll_records=50_000` and `max_partition_fetch_bytes=10MB`** are needed to
   amortise the per-poll RTT over as many records as possible.

5. **`FuturesUnordered` alone does not make sends concurrent** unless it is polled
   continuously (e.g., via `tokio::select!`). A naïve `while let Some(r) = futs.next().await`
   drain loop serialises rather than pipelines.

## 5. Comparison vs. `stream-test-rust` (rdkafka)

| Aspect | stream-test-krafka | stream-test-rust (rdkafka) |
|---|---|---|
| C dependency | None | Yes (librdkafka, cmake) |
| Cross-compilation | Pure Rust (`cargo build --target …`) | Requires C cross-compiler |
| Producer send model | Async, ACK-gated | Sync enqueue into C ring buffer |
| Consumer fetch model | Sync per-poll network fetch | Background C prefetch thread |
| Observed throughput (8 workers, local broker) | ~185k msg/sec (`Acks::None`) | ~3.5M msg/sec |
| Throughput gap closable by app tuning? | No — architectural limit in krafka v0.7 | N/A |

## 6. Implementation Checklist

- [x] `processor.rs` — Phase 1 JoinSet load + Phase 2 channel-decoupled consume/produce loop, per-OS-thread runtime
- [x] `producer.rs` — standalone producer for normal (non-benchmark) mode
- [x] `consumer.rs` — standalone consumer for normal (non-benchmark) mode
- [x] Inline Zig-Zag encode/decode (`read_varint` / `write_varint`)
- [x] Progress reporter (1-second interval)
- [x] Final result log: `BENCHMARK RESULT (KRAFKA)`, total records, seconds, msg/sec
- [x] `Cargo.toml` — krafka 0.7 (published), tokio, futures, rand, env_logger
- [x] `Makefile` — `run-benchmark`, `docker-build`, `docker-run-benchmark`
- [x] `Dockerfile` — multi-stage Alpine build
