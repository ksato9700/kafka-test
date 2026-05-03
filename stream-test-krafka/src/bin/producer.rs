use krafka::producer::Producer;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use std::env;

#[inline(always)]
fn write_varint(buf: &mut Vec<u8>, n: i64) {
    let mut val = ((n << 1) ^ (n >> 63)) as u64;
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
