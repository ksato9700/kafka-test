use krafka::consumer::{AutoOffsetReset, Consumer};
use std::env;
use std::time::Duration;

#[inline(always)]
fn read_varint(reader: &mut &[u8]) -> i64 {
    let mut val: u64 = 0;
    let mut shift = 0;
    loop {
        let b = reader[0];
        *reader = &(*reader)[1..];
        val |= ((b & 0x7F) as u64) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            break;
        }
    }
    ((val >> 1) as i64) ^ -((val & 1) as i64)
}

fn main() {
    env_logger::init();
    let bootstrap = env::var("BOOTSTRAP").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = "integer-sum-output";

    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(async {
        let consumer = Consumer::builder()
            .bootstrap_servers(&bootstrap)
            .group_id("stream-test-krafka-consumer")
            .auto_offset_reset(AutoOffsetReset::Earliest)
            .enable_auto_commit(true)
            .build()
            .await
            .expect("Failed to create consumer");

        consumer
            .subscribe(&[topic])
            .await
            .expect("Failed to subscribe");

        println!("📥 Consumer listening on '{}'...", topic);

        loop {
            match consumer.poll(Duration::from_millis(100)).await {
                Ok(records) => {
                    for record in records {
                        if let Some(ref value_bytes) = record.value {
                            let mut payload: &[u8] = value_bytes.as_ref();
                            let sum = read_varint(&mut payload);
                            println!("Sum: {}", sum);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Poll error: {:?}", e);
                }
            }
        }
    });
}
