# Design Proposal: Hardening krafka Producer Concurrency for Idempotent Sends

## 1. Summary

`kafka-test-krafka` and `stream-test-krafka` are safe today because every concurrent
producer path in this repo runs with `idempotent(false)` / `Acks::None`. That
configuration was chosen for maximum benchmark throughput, but it also happens to be
the reason a real correctness bug in one of the concurrency patterns has never
surfaced here. If either codebase later enables `idempotent(true)` (the krafka builder
default, and the setting needed for a "durable" or "production-representative"
benchmark mode) without also changing the concurrency pattern, `stream-test-krafka`'s
Phase 1 loader will intermittently fail with a broker-level `OutOfOrderSequenceNumber`
error under load.

This finding comes from a sibling project (`tiered-iceberg-poc`, a separate local
repo, not published here) that hit this exact failure while building a
similarly-shaped krafka producer, root-caused it, and fixed it. This document adapts
that finding to this repo's two krafka codebases and proposes concrete changes. It
does not modify any code — per request, all recommendations below are proposals for a
future change, not applied edits.

## 2. The underlying bug (recap)

An idempotent Kafka producer must send every batch to a given partition with a
strictly increasing sequence number; the broker rejects any batch that arrives out of
sequence with `OutOfOrderSequenceNumber`. krafka allocates that sequence number
atomically **inside** `Producer::send()` / `send_record()`, at the point the future is
first polled.

`tokio::task::JoinSet::spawn()` schedules each pushed future as an independent task.
Tokio's scheduler does **not** guarantee tasks run in spawn order — under load, two
sends targeting the same partition can have their sequence numbers allocated in the
opposite order from how they were spawned. With `idempotent(false)` this is harmless
(there is no sequence number to violate). With `idempotent(true)` it is a real,
broker-visible correctness bug, confirmed at both 3,000-wide and 1,000-wide
`JoinSet` windows — it is a probabilistic race, not a size threshold that can be tuned
around.

The fix that was validated (300,000 messages against a real broker, zero errors, full
`idempotent(true)`/`Acks::All`) is to drive concurrent sends from a **single task**
via `futures::stream::FuturesUnordered`, pushed and polled from that one task with no
`tokio::spawn` involved. This preserves push-order initiation because nothing else is
scheduling those futures independently.

**Critical corollary, also confirmed the hard way:** `FuturesUnordered` alone is not
sufficient — it only preserves push-order initiation if `send()`/`send_record()` is
the pushed future's **only** await point. If the future does anything else awaitable
first (e.g. a schema-registry encode call), that earlier await can resolve out of
push order across concurrently-pushed futures, and the same `OutOfOrderSequenceNumber`
error reappears even though the concurrency primitive is no longer `JoinSet`. Neither
codebase in this repo currently has a pre-send await in the hot path (see §3), but any
future change that adds one (e.g. swapping the manual Zig-Zag encoding for something
that calls out to a registry, cache, or other async resource) would need to keep that
work outside the concurrent phase.

## 3. Current state in this repo

| Location | Concurrency model | `idempotent` | At risk today? | Latent risk? |
|---|---|---|---|---|
| `kafka-test-krafka/src/producer.rs` | None — one `send().await` every 2s | default (`true`) | No — no concurrency to race | No |
| `stream-test-krafka` Phase 1 loader (`processor.rs:133-159`) | `JoinSet::spawn()`, 10,000-wide window | explicitly `false` | No — idempotency disabled | **Yes** — breaks immediately if `idempotent(true)` is set without also changing this loop |
| `stream-test-krafka` Phase 2 per-worker pipeline (`processor.rs:249-280`) | Single-task `FuturesUnordered` behind an `mpsc` channel | explicitly `false` | No | Structurally safe already (single await point, no pre-send encode step), but this has never been exercised under `idempotent(true)` in this repo, so it is unverified, not proven |

Two observations worth calling out explicitly:

- **`kafka-test-krafka/src/producer.rs` is fine as-is.** It sends one message every
  two seconds with a plain sequential `await` — there is no concurrency, so there is
  nothing for this bug class to exploit. No change needed there.
- **Phase 2's existing architecture is *already* the pattern we would recommend**,
  even though the code comments (`processor.rs:247-248`) explain it purely as "don't
  block the consume loop on producer I/O" — there's no mention of idempotent-producer
  sequencing anywhere in `stream-sum-krafka-design.md`. That's a coincidence worth
  documenting rather than relying on: the next person who touches Phase 2 has no
  signal that its shape is load-bearing for a durability property, not just a
  latency-hiding trick.
- **Phase 1's `JoinSet` loop is the one genuine gap.** It is safe under the current
  `idempotent(false)` configuration, but nothing in the code or docs marks that
  safety as *conditional* on that setting staying `false`.

## 4. Recommendations

### 4.1 Guard the hazard where it lives (no behavior change)

Add a comment directly above the `JoinSet` loop in `processor.rs` (Phase 1) stating
that this pattern is unsafe if `idempotent(true)` is ever set on this producer, with a
pointer to §4.2 for the safe alternative. This costs nothing and prevents the most
likely failure mode: someone flips `idempotent(false)` → default/`true` while chasing
a durability requirement, without realizing the concurrency primitive needs to change
too.

### 4.2 Migrate Phase 1's loader from `JoinSet` to single-task `FuturesUnordered`

This makes Phase 1 structurally consistent with Phase 2 and removes the latent hazard
entirely, independent of whatever `idempotent`/`Acks` setting Phase 1 ends up using.
Sketch (illustrative, not a diff to apply):

```rust
let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
for _ in 0..TOTAL_RECORDS {
    // ... encode into buf as today ...
    let p = Arc::clone(&producer);
    let data = buf.clone();
    // Pushed, not spawned — polled only by this task via `in_flight.next()`.
    in_flight.push(async move { p.send("integer-list-input-benchmark", None, &data).await });
    if in_flight.len() >= WINDOW {
        in_flight.next().await;
    }
}
while in_flight.next().await.is_some() {}
```

This is expected to be throughput-neutral or better versus `JoinSet` — it removes
10,000-wide task-spawn overhead, which `stream-sum-krafka-design.md`'s own "What was
tried" table (row 3) already identifies as a measurable cost (`tokio::spawn` per send
was explicitly abandoned in the v0.7 exploration for this reason). Re-measuring Phase 1
throughput after this change would be a useful, low-cost confirmation.

### 4.3 Add a "durable mode" benchmark variant

`BENCHMARK_SUMMARY.md` already documents the throughput cost of different
durability settings for krafka v0.7 (`Acks::None` vs `Acks::Leader`). Extending that
comparison to a full `idempotent(true)`/`Acks::All` run — using the hardened
`FuturesUnordered` pattern from §4.2 in both phases — would answer a question the
current benchmark can't: what does this pure-Rust client cost in throughput for the
delivery guarantee most production pipelines actually need, not just the best-effort
ceiling. This also happens to be the only way to positively confirm the fix in this
repo's own environment (Docker/Lima broker, local RTTs) rather than relying solely on
the sibling project's dev-staging measurements.

Suggested shape: an env var (e.g. `DURABLE=1`) that switches both phases'
`Producer::builder()` calls to drop `.idempotent(false)` / `.acks(Acks::None)` (falling
back to the builder defaults, `idempotent(true)`/`Acks::All`), and a corresponding
`make run-benchmark-durable` target. Report both numbers side by side in
`BENCHMARK_SUMMARY.md`, matching the existing table's format.

### 4.4 Document the finding in `stream-sum-krafka-design.md`

That document already has a numbered "Key tuning lessons" list (§4, items 1-8) that is
exactly the right home for this. Suggested addition as item 9, once §4.2 lands:

> 9. **`JoinSet::spawn()` is unsafe with `idempotent(true)`.** Sequence-number
>    allocation happens atomically inside `send()`, but tokio doesn't guarantee
>    spawned tasks run in spawn order — concurrently spawned sends to the same
>    partition can have their sequence numbers allocated out of order, and the broker
>    rejects the result with `OutOfOrderSequenceNumber`. Use a single-task
>    `FuturesUnordered` instead (as Phase 2 already does) any time durability beyond
>    `idempotent(false)` is required — and keep `send()`/`send_record()` as that
>    future's only await point; an async encode/registry step ahead of it reintroduces
>    the same race through a different door.

## 5. Non-goals

- **This is not a bug report against the current code.** Both `stream-test-krafka`
  phases run with `idempotent(false)` today, so `OutOfOrderSequenceNumber` cannot
  occur in the current benchmark configuration. Everything above is forward-looking
  hardening, prompted by a real failure seen in a different codebase with the same
  library and a similar pattern.
- **Not proposing to change the default max-throughput benchmark's semantics.** §4.3
  is additive (a new mode/target), not a replacement for the existing best-effort
  numbers already published in `BENCHMARK_SUMMARY.md`.
- **Not addressing the separate, already-documented rdkafka throughput gap** (the
  ~5x-19x gaps discussed at length in `stream-sum-krafka-design.md` §4-5). That is a
  different, already-tracked topic; this document is scoped to the idempotent-producer
  concurrency hazard only.

## 6. Verification plan (if this is picked up)

Following this project's own "prove it against the real thing" norm (and the exact
methodology that surfaced the original bug):

1. Build a small standalone example that pushes N sends through `FuturesUnordered`
   with `idempotent(true)`/`Acks::All` against the local Docker/Lima broker, no other
   application logic — confirm zero `OutOfOrderSequenceNumber` at a scale well above
   this repo's typical window sizes (e.g. 300K, matching the sibling project's proof
   scale).
2. Apply the same shape to Phase 1's loader (§4.2), rebuild, and run the existing
   50M-record benchmark with `DURABLE=1` (§4.3) — confirm zero errors end-to-end, not
   just in the standalone example.
3. Record the durable-mode throughput number alongside the existing best-effort number
   in `BENCHMARK_SUMMARY.md`, and add the lesson to `stream-sum-krafka-design.md`
   (§4.4) only after the run confirms it — don't document a fix that hasn't been
   exercised in this repo's own environment.
