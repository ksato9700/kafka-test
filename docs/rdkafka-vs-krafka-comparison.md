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

**When to choose krafka:** New projects targeting Kafka 3.9+, CI environments where installing cmake/g++ is painful, or anywhere a simpler dependency tree is valued — **provided throughput is not the priority**.

**When to choose rdkafka:** Any streaming or high-throughput workload, projects that need to support older brokers, or those that rely on topic auto-creation.

---

## Streaming Throughput

Beyond the API comparison above, this repository includes `stream-test-krafka` — a streaming benchmark that mirrors the Java/C/Go/rdkafka benchmarks: produce 50 million integer-list records, then process them with 8 workers (read → sum → write).

### Results (Apple M4, macOS 26.4.1, Kafka 4.2.0)

| Implementation | Throughput | Processing Time |
|---|---|---|
| Java (Kafka Streams) | 6,312,468 msg/sec | 7.92 s |
| C (librdkafka) | 4,719,502 msg/sec | 10.59 s |
| **Rust (rdkafka)** | **3,499,934 msg/sec** | **14.29 s** |
| Go (confluent-kafka-go) | 2,447,551 msg/sec | 20.43 s |
| **Rust (krafka 0.7)** | **28,710 msg/sec** | **1741.55 s** |

krafka is **~120× slower** than rdkafka and **~220× slower** than Java on this workload.

### Root Cause: No Fire-and-Forget Producer

The gap is not a tuning issue — it is architectural. The two clients have fundamentally different producer delivery models:

**rdkafka (`ThreadedProducer`)** runs a background C thread that manages batching, compression, retries, and broker acknowledgements independently of the application thread. The application calls `produce()` and returns immediately; the record is enqueued internally. The background thread drains the queue and confirms delivery via a callback. This means the application loop never waits for the broker.

**krafka** has no equivalent. Its `producer.send(...).await` is a true async future that completes only when the broker has acknowledged the message. There is no background thread, no internal queue, and no batch/pipeline API. Every send is a round-trip.

In a stream processor, this matters at every message:

```
rdkafka:  consume → process → enqueue (μs) → next record immediately
krafka:   consume → process → send → await ACK (≥ RTT) → next record
```

### What We Tried

**Sequential `.await` (baseline):** Each worker called `producer.send(...).await` per message. Result: **15,101 msg/sec**.

**`futures::join_all` over a poll batch:** Collect all output futures for a batch, then drive them all concurrently with `join_all`. This roughly halves send latency per batch. Result: **28,710 msg/sec** (~1.9× improvement).

The `join_all` approach is the best achievable within krafka's API. Further improvement would require krafka to expose a queued/fire-and-forget send path, which it currently does not.

### Conclusion

krafka is well-suited for **low-throughput, latency-tolerant** workloads: administrative tools, low-volume event pipelines, CLI utilities, and applications where the simplicity of pure Rust dependencies outweighs raw throughput. It is not suitable for stream-processing workloads where each worker must process thousands to millions of messages per second.
