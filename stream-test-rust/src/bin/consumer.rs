use rdkafka::config::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::message::Message;
use std::env;
use std::time::Duration;

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
    }
    ((val >> 1) as i64) ^ -((val & 1) as i64)
}

fn main() {
    let bootstrap_servers = env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("OUTPUT_TOPIC").unwrap_or_else(|_| "integer-sum-output-avro".to_string());

    let consumer: BaseConsumer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("broker.address.family", "v4")
        .set("group.id", "rust-test-consumer-group")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "earliest")
        .create()
        .expect("Consumer creation error");

    consumer.subscribe(&[&topic]).expect("Can't subscribe");

    println!("🚀 Waiting for Avro results on topic: {}...", topic);

    loop {
        if let Some(Ok(m)) = consumer.poll(Duration::from_millis(100)) {
            if let Some(mut payload) = m.payload() {
                let sum = read_varint(&mut payload);
                println!("Received Avro Result: sum={}", sum);
            }
        }
    }
}
