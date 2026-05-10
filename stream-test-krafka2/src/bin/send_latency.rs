/// Micro-benchmark: isolates two potential bottlenecks in producer send patterns.
///
/// Bottleneck 1 — Linger sequential trap:
///   send_record().await blocks until the batch flushes (≈linger_ms per call).
///   Sequential loop = stop-and-wait at ~1/linger_ms records/sec.
///
/// Bottleneck 2 — max_in_flight semaphore:
///   Even with FuturesUnordered, if the producer's internal semaphore has capacity N,
///   the (N+1)th push blocks immediately — reverting to near-sequential throughput.
///   krafka default: max_in_flight=5 (idempotent constraint); non-idempotent default unknown.
///
/// Test matrix:
///   A) Sequential await, N=1000                (confirms linger trap; capped to finish quickly)
///   B) FuturesUnordered w=1000, default max_in_flight
///   C) FuturesUnordered w=1000, max_in_flight=1000  (removes semaphore if default is low)
///   D) FuturesUnordered w=1000, max_in_flight=5000
///   E) FuturesUnordered w=1000, linger=0ms, default max_in_flight  (removes linger trap)
///
/// Run: BOOTSTRAP=127.0.0.1:9094 cargo run --release --bin send_latency
use futures::stream::{FuturesUnordered, StreamExt};
use krafka::producer::{Acks, Producer, ProducerRecord};
use std::env;
use std::time::{Duration, Instant};

const N_SEQ: usize = 1_000;
const N: usize = 100_000;
const TOPIC: &str = "integer-sum-output-benchmark";
const LINGER_MS: u64 = 5;
const BATCH_BYTES: usize = 64 * 1024;

async fn make_producer_with(bootstrap: &str, linger_ms: u64, max_in_flight: usize) -> Producer {
    Producer::builder()
        .bootstrap_servers(bootstrap)
        .acks(Acks::None)
        .idempotent(false)
        .linger(Duration::from_millis(linger_ms))
        .batch_size(BATCH_BYTES)
        .max_in_flight(max_in_flight)
        .build()
        .await
        .expect("Failed to create producer")
}

fn make_payload() -> Vec<u8> {
    vec![0x02, 0x54, 0x00]
}

async fn run_sequential(producer: &Producer, n: usize) -> f64 {
    let payload = make_payload();
    let _ = producer.send_record(ProducerRecord::new(TOPIC, payload.clone())).await;
    let start = Instant::now();
    for _ in 0..n {
        let _ = producer.send_record(ProducerRecord::new(TOPIC, payload.clone())).await;
    }
    n as f64 / start.elapsed().as_secs_f64()
}

async fn run_futures_unordered(producer: &Producer, n: usize, window: usize) -> f64 {
    let payload = make_payload();
    let _ = producer.send_record(ProducerRecord::new(TOPIC, payload.clone())).await;
    let start = Instant::now();
    let mut futs: FuturesUnordered<_> = FuturesUnordered::new();
    for _ in 0..n {
        futs.push(producer.send_record(ProducerRecord::new(TOPIC, payload.clone())));
        if futs.len() >= window {
            futs.next().await;
        }
    }
    while futs.next().await.is_some() {}
    n as f64 / start.elapsed().as_secs_f64()
}

#[tokio::main]
async fn main() {
    let bootstrap = env::var("BOOTSTRAP").unwrap_or_else(|_| "127.0.0.1:9094".to_string());

    println!("=== send_latency micro-benchmark ===");
    println!("N_SEQ={N_SEQ}, N={N}, linger={LINGER_MS}ms, batch_size={BATCH_BYTES}B");
    println!("Topic: {TOPIC}");
    println!();

    // A) Sequential, default max_in_flight — baseline for the linger trap
    {
        let p = make_producer_with(&bootstrap, LINGER_MS, 5).await;
        let tput = run_sequential(&p, N_SEQ).await;
        println!("A) Sequential (N={N_SEQ}, linger={LINGER_MS}ms, mif=default):  {tput:>10.0} msg/sec");
        p.close().await;
    }

    // B) FuturesUnordered w=1000, default max_in_flight=5
    //    If semaphore capacity=5, the 6th push blocks → effectively sequential
    {
        let p = make_producer_with(&bootstrap, LINGER_MS, 5).await;
        let tput = run_futures_unordered(&p, N, 1000).await;
        println!("B) FuturesUnordered w=1000  (linger={LINGER_MS}ms, mif=5):     {tput:>10.0} msg/sec");
        p.close().await;
    }

    // C) FuturesUnordered w=1000, max_in_flight=1000
    //    Removes semaphore bottleneck; linger still applies unless batch fills
    {
        let p = make_producer_with(&bootstrap, LINGER_MS, 1000).await;
        let tput = run_futures_unordered(&p, N, 1000).await;
        println!("C) FuturesUnordered w=1000  (linger={LINGER_MS}ms, mif=1000):  {tput:>10.0} msg/sec");
        p.close().await;
    }

    // D) FuturesUnordered w=1000, max_in_flight=5000
    //    Even higher semaphore headroom; should match or exceed C
    {
        let p = make_producer_with(&bootstrap, LINGER_MS, 5000).await;
        let tput = run_futures_unordered(&p, N, 1000).await;
        println!("D) FuturesUnordered w=1000  (linger={LINGER_MS}ms, mif=5000):  {tput:>10.0} msg/sec");
        p.close().await;
    }

    // E) FuturesUnordered w=1000, linger=0ms, max_in_flight=5000
    //    Removes both bottlenecks; shows the ceiling without linger or semaphore
    {
        let p = make_producer_with(&bootstrap, 0, 5000).await;
        let tput = run_futures_unordered(&p, N, 1000).await;
        println!("E) FuturesUnordered w=1000  (linger=0ms,  mif=5000):  {tput:>10.0} msg/sec");
        p.close().await;
    }

    println!();
    println!("Interpretation guide:");
    println!("  A ≈ B:        semaphore is the bottleneck in B (mif too low)");
    println!("  B << C ≈ D:   raising mif fixes concurrency; linger no longer limiting");
    println!("  C ≈ D >> A:   both mif and linger are addressed by concurrent enqueue");
    println!("  E >> C:       linger=5ms is still a ceiling even with mif=1000");
    println!("  E ≈ C:        batch fills fast enough that linger never fires");
}
