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

**When to choose krafka:** New projects targeting Kafka 3.9+, CI environments where installing cmake/g++ is painful, or anywhere a simpler dependency tree is valued.

**When to choose rdkafka:** Projects that need to support older brokers, rely on topic auto-creation, or prefer the stream-based consumer API.
