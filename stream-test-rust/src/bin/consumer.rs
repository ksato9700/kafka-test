use apache_avro::{from_avro_datum, types::Value, Schema};
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::Message;
use std::env;
use std::fs;
use std::sync::Arc;

fn load_schema(file_name: &str) -> Schema {
    let paths = [format!("../schemas/{}", file_name), format!("schemas/{}", file_name)];
    for path in paths {
        if let Ok(content) = fs::read_to_string(&path) {
            return Schema::parse_str(&content).expect("Failed to parse schema");
        }
    }
    panic!("Schema file not found: {}", file_name);
}

#[tokio::main]
async fn main() {
    let bootstrap_servers = env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("OUTPUT_TOPIC").unwrap_or_else(|_| "integer-sum-output-avro".to_string());
    let schema = Arc::new(load_schema("sum_result.avsc"));

    let consumer: StreamConsumer = ClientConfig::new()
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
        match consumer.recv().await {
            Ok(m) => {
                if let Some(payload) = m.payload() {
                    let mut reader = payload;
                    if let Ok(Value::Record(fields)) = from_avro_datum(&schema, &mut reader, None) {
                        for (name, val) in fields {
                            if name == "sum" {
                                if let Value::Long(s) = val {
                                    println!("Received Avro Result: sum={}", s);
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("Consumer error: {}", e),
        }
    }
}
