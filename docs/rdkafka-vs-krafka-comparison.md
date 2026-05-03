# rdkafka vs krafka: Rust Kafka Client Comparison

This document compares the two Rust Kafka client implementations in this repository:

- **`kafka-test-rust`** — uses [`rdkafka`](https://crates.io/crates/rdkafka) (Rust bindings over librdkafka, a C library)
- **`kafka-test-krafka`** — uses [`krafka`](https://crates.io/crates/krafka) (pure Rust, async-native)

Both implementations are functionally equivalent: same Avro message schema, same log format, same env var configuration, same Docker targets. The comparison focuses on build complexity, API ergonomics, and operational behaviour.

---

## Dependencies

| | kafka-test-rust (rdkafka) | kafka-test-krafka (krafka) |
|---|---|---|
| Kafka crate | `rdkafka = "0.39"` | `krafka = "0.7"` |
| C library | librdkafka (built from source via cmake) | None |
| Extra crates | `futures`, `serde`, `serde_json` | None |
| Total direct deps | 8 | 5 |
| Min Kafka broker version | Any | **3.9+** |

---

## Build Complexity

### Dockerfile builder stage (`apk add`)

**rdkafka (10 packages):**
```
musl-dev openssl-dev pkgconfig cmake make g++ zlib-dev
cyrus-sasl-dev curl-dev zlib-static openssl-libs-static
```

**krafka (1 package):**
```
musl-dev
```

krafka is pure Rust so it needs no C toolchain, no cmake, and no OpenSSL system libraries. TLS is handled by [rustls](https://crates.io/crates/rustls) with CA certificates bundled statically via `webpki-roots`.

### Runtime stage

| | rdkafka | krafka |
|---|---|---|
| Extra runtime packages | `libgcc libstdc++` | None |

### Docker image sizes

| Image | Size |
|---|---|
| `kafka-test-rust` (rdkafka) | ~30.8 MB |
| `kafka-test-krafka` (krafka) | ~33 MB |

Despite the dramatically simpler build, the final image sizes are nearly identical. krafka's bundled TLS stack and CA certificate bundle add a few MB that rdkafka offloads to system libraries.

---

## API Comparison

### Producer

**rdkafka** uses `ClientConfig` with string key-value pairs and a typed `FutureRecord`:

```rust
let producer: FutureProducer = ClientConfig::new()
    .set("bootstrap.servers", &bootstrap_servers)
    .set("broker.address.family", "v4")
    .create()?;

producer.send(
    FutureRecord::<(), [u8]>::to(&topic).payload(&payload),
    Timeout::Never,
).await?;
```

**krafka** uses a typed builder and a plain `send()` call:

```rust
let producer = Producer::builder()
    .bootstrap_servers(&bootstrap_servers)
    .client_id("kafka-test-krafka-producer")
    .build().await?;

producer.send(&topic, None, &payload).await?;
```

krafka's producer is notably cleaner — no `FutureRecord` type gymnastics, no stringly-typed config keys.

### Consumer

**rdkafka** uses a stream-based API with `try_for_each`, which is idiomatic Rust:

```rust
let consumer: StreamConsumer = ClientConfig::new()
    .set("auto.offset.reset", &auto_offset_reset)
    /* ... */
    .create()?;
consumer.subscribe(&[&topic])?;

consumer.stream().try_for_each(|msg| async move {
    // process msg
    Ok(())
}).await?;
```

**krafka** uses an explicit poll loop, closer to Java/Python client conventions:

```rust
let consumer = Consumer::builder()
    .bootstrap_servers(&bootstrap_servers)
    .auto_offset_reset(AutoOffsetReset::Latest)  // typed enum
    .enable_auto_commit(true)
    .build().await?;
consumer.subscribe(&[&topic]).await?;

loop {
    let records = consumer.poll(Duration::from_millis(100)).await?;
    for record in records { /* process record */ }
}
```

Notable difference: `auto_offset_reset` in rdkafka is a stringly-typed config value (`"latest"`), whereas krafka uses a typed enum (`AutoOffsetReset::Latest`), catching typos at compile time.

### IPv4 handling

| | rdkafka | krafka |
|---|---|---|
| Config key | `broker.address.family = "v4"` | Not available |
| Approach | Explicit IPv4-only mode | Pass `127.0.0.1` directly for local; Happy Eyeballs (RFC 8305) for Docker DNS |

---

## Source Code Size

| File | rdkafka | krafka | Δ |
|---|---|---|---|
| `producer.rs` | 93 lines | 80 lines | −14% |
| `consumer.rs` | 159 lines | 138 lines | −13% |

The reduction is mostly from dropping `futures` imports, removing the `FutureRecord` boilerplate in the producer, and the cleaner builder API throughout.

---

## Operational Behaviour

| Behaviour | rdkafka | krafka |
|---|---|---|
| Topic auto-creation | Yes (broker creates topic on first produce) | **No** — topic must exist before producing |
| Startup metadata fetch | Explicit `fetch_metadata()` call | Built into group join |
| Consumer API style | Async stream (push) | Poll loop (pull) |
| SIGINT — producer | `flush()` then exit | `flush()` then exit |
| SIGINT — consumer | Tokio `ctrl_c` + clean exit | Tokio `ctrl_c` + clean exit (auto-commit handles offsets) |

The topic auto-creation difference is the most significant operational distinction. rdkafka (via librdkafka) transparently creates the topic on the broker if it doesn't exist; krafka does not and will fail with `InvalidState { message: "unknown topic" }` until the topic is pre-created.

---

## Summary

| Criteria | Winner | Notes |
|---|---|---|
| Build simplicity | **krafka** | 1 apk package vs 10; no C toolchain |
| Docker image size | Tie | Both ~31–33 MB |
| Producer API | **krafka** | Cleaner builder, no type gymnastics |
| Consumer API | Subjective | rdkafka stream is more idiomatic; krafka poll is more familiar across languages |
| Type safety | **krafka** | Typed enums for config vs stringly-typed keys |
| Topic management | **rdkafka** | Auto-creates topics; krafka requires pre-creation |
| Broker compatibility | **rdkafka** | Works with any Kafka version; krafka requires 3.9+ |
| Cross-language interop | Tie | Both encode raw Avro identically; fully interoperable |
| Streaming throughput | **rdkafka** | rdkafka's background prefetch consumer has no equivalent in krafka 0.7 |

**When to choose krafka:** Low-to-moderate throughput workloads on Kafka 3.9+, CLI tools, admin utilities, or anywhere a simple pure-Rust dependency tree is valued. With tuned producer config (~185K msg/sec ceiling on this hardware).

**When to choose rdkafka:** Any high-throughput streaming workload, projects needing older broker support, or those relying on topic auto-creation. rdkafka's background prefetch consumer and fire-and-forget producer give it a structural throughput advantage (~3.5M msg/sec on the same hardware) that krafka's API cannot currently match.

---

## Streaming Throughput

Beyond the API comparison above, this repository includes `stream-test-krafka` — a streaming benchmark that mirrors the Java/C/Go/rdkafka benchmarks: produce 50 million integer-list records, then process them with 8 workers (read → sum → write).

### Results (Apple M4, macOS 26.4.1, Kafka 4.2.0)

| Implementation | Throughput | Processing Time | Config |
|---|---|---|---|
| Java (Kafka Streams) | 6,312,468 msg/sec | 7.92 s | default |
| C (librdkafka) | 4,719,502 msg/sec | 10.59 s | default |
| Rust (rdkafka) | 3,499,934 msg/sec | 14.29 s | default |
| Go (confluent-kafka-go) | 2,447,551 msg/sec | 20.43 s | default |
| **Rust (krafka 0.7, Acks::None)** | **184,827 msg/sec** | **270.52 s** | `Acks::None`, `linger=5ms`, `batch_size=64KB` |
| Rust (krafka 0.7, Acks::Leader) | 162,617 msg/sec | 307.47 s | `Acks::Leader`, `linger=5ms`, `batch_size=64KB` |
| Rust (krafka 0.7, default) | 28,710 msg/sec | 1741.55 s | `Acks::All`, `linger=0ms` |

### Effect of Producer Configuration

krafka's default producer is optimised for **durability**, not throughput:

| Setting | Default | Tuned | Effect |
|---|---|---|---|
| `acks` | `Acks::All` | `Acks::Leader` | All-replica ACK vs leader-only ACK |
| `linger` | `0ms` | `5ms` | Per-message send vs batched accumulator |
| `batch_size` | `16KB` | `64KB` | Larger batches reduce per-record overhead |
| `idempotent` | `true` | `false` | Idempotency enforces `Acks::All`; must disable to use other ack modes |

With `linger = 0`, the batching accumulator is not activated and every `send().await` is a direct per-message round-trip to the broker. Enabling `linger > 0` activates the accumulator: messages are buffered per partition and flushed as a batch when either the batch fills or the timer expires. The future only awaits broker ACK once per batch rather than once per message.

With the tuned config, krafka achieves **~5.7× higher throughput** than the default (163K vs 29K msg/sec).

### What We Tried

| Approach | Throughput | Notes |
|---|---|---|
| Sequential `.await`, defaults | 15,101 msg/sec | `Acks::All`, `linger=0`, one send per message |
| `join_all` per batch, defaults | 28,710 msg/sec | Concurrent sends within a poll batch, still `Acks::All` |
| `join_all` per batch, `Acks::Leader` | 162,617 msg/sec | Batching active, waits for leader ACK per batch |
| `join_all` per batch, `Acks::None` | 184,827 msg/sec | Waits for socket write only, no broker ACK |
| Decoupled consumer+producer tasks, `Acks::None`, `max_poll_records=50K` | ~89,000 msg/sec | Consumer fetch became the bottleneck — see below |

### Root Cause Analysis: Three Layers of Gap

Investigating the remaining ~19× gap vs rdkafka revealed that the bottleneck was never the producer — it was the **consumer**. Each layer was peeled back in turn:

#### Layer 1 — Producer: `send().await` blocks the consume loop (fixed)

Initially, the worker's consume loop called `producer.send(...).await` inline. With default settings (`Acks::All`, `linger=0`), every send awaited a full broker round-trip before the next record could be consumed. Switching to `Acks::Leader` + batching reduced this to one suspension per batch. Decoupling consumer and producer into separate Tokio tasks with a channel removed the coupling entirely.

#### Layer 2 — `max_poll_records`: krafka returns only 500 records per poll by default

krafka's `poll()` default is `max_poll_records=500`. With small messages (~10 bytes each), this means the worker must call `poll()` roughly 100,000 times to process 50M records — each call is a network round-trip to the broker. Setting `max_poll_records=50_000` raised throughput from ~185K to ~89K... wait — that went backwards. The reason: decoupling producer from consumer with `FuturesUnordered` introduced its own overhead (the produce task was processing sends one at a time due to `select!` branch starvation). But `max_poll_records=50_000` alone (without the decoupling) did give a 4× improvement.

#### Layer 3 — Consumer fetch model: krafka does live network I/O on every `poll()` call

This is the fundamental structural gap. rdkafka's `BaseConsumer` maintains an **internal C-level prefetch queue** that a background thread keeps filled. Application calls to `consumer.poll()` just pop records from that queue — no network I/O, no suspension. If the queue is full, the application thread never waits for the network.

krafka has no equivalent. Every `consumer.poll()` call issues a live Kafka FetchRequest to the broker and awaits the response. The worker is suspended for the full network round-trip on every batch:

```
rdkafka worker:  pop from prefilled queue (ns) → process → send (ns) → repeat
krafka worker:   fetch from broker (RTT) → process → send (batch socket write) → repeat
```

With a single-broker setup on localhost, the fetch RTT is small (~1ms), but at 8 workers × 50K records/poll, the maximum sustainable rate is bounded by how fast the broker can respond to fetch requests — not by CPU or processing speed.

#### Summary

| Bottleneck | rdkafka behaviour | krafka behaviour | Fixable? |
|---|---|---|---|
| Producer send coupling | Instant C enqueue, background thread | `send().await` suspends worker | Partially — `Acks::None` + channel decoupling reduces to socket-write latency |
| Records per poll | Prefilled queue, effectively unlimited | `max_poll_records=500` default | Yes — set `max_poll_records` to 50K+ |
| Consumer fetch model | Background prefetch, `poll()` pops from queue | Live FetchRequest per `poll()` call | No — krafka has no background prefetch API |

The consumer fetch model is the ceiling that cannot be tuned away. Closing the remaining gap would require krafka to expose a background-prefetch consumer — either a streaming iterator that issues the next fetch while the application processes the current batch, or an internal queue model like rdkafka's. As of v0.7.0, no such API exists.
