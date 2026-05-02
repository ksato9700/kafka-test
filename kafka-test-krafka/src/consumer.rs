use apache_avro::Schema;
use apache_avro::types::Value;
use krafka::consumer::{AutoOffsetReset, Consumer};
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn load_schema() -> Schema {
    let paths = [
        "../schemas/message.avsc",
        "/app/schemas/message.avsc",
        "schemas/message.avsc",
    ];
    for p in paths {
        if Path::new(p).exists() {
            let content = fs::read_to_string(p).expect("Failed to read schema file");
            return Schema::parse_str(&content).expect("Failed to parse schema");
        }
    }
    panic!("Schema file not found in paths: {:?}", paths);
}

fn parse_offset_reset(s: &str) -> AutoOffsetReset {
    match s {
        "earliest" => AutoOffsetReset::Earliest,
        "none" => AutoOffsetReset::None,
        _ => AutoOffsetReset::Latest,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let schema = load_schema();
    tracing::info!("Loaded Avro schema");

    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());
    let group_id = env::var("CONSUMER_GROUP_ID").unwrap_or_else(|_| "my-group-1".to_string());
    let auto_offset_reset_str =
        env::var("AUTO_OFFSET_RESET").unwrap_or_else(|_| "latest".to_string());
    let auto_offset_reset = parse_offset_reset(&auto_offset_reset_str);

    tracing::info!(
        "🛠️ Connecting KafkaConsumer to topic '{}' at '{}' (group: '{}')...",
        topic,
        bootstrap_servers,
        group_id
    );

    let consumer = Consumer::builder()
        .bootstrap_servers(&bootstrap_servers)
        .group_id(&group_id)
        .auto_offset_reset(auto_offset_reset)
        .enable_auto_commit(true)
        .build()
        .await
        .expect("Failed to create consumer");

    consumer
        .subscribe(&[topic.as_str()])
        .await
        .expect("Failed to subscribe to topic");

    tracing::info!("📥 Listening for messages...");

    loop {
        tokio::select! {
            result = consumer.poll(Duration::from_millis(100)) => {
                match result {
                    Ok(records) => {
                        for record in records {
                            if let Some(ref value_bytes) = record.value {
                                let mut reader = value_bytes.as_ref();
                                match apache_avro::from_avro_datum(&schema, &mut reader, Some(&schema)) {
                                    Ok(Value::Record(fields)) => {
                                        let message_id = fields
                                            .iter()
                                            .find(|(k, _)| k == "message_id")
                                            .and_then(|(_, v)| match v {
                                                Value::Long(l) => Some(*l),
                                                Value::Int(i) => Some(*i as i64),
                                                _ => None,
                                            })
                                            .unwrap_or(0);

                                        let event_time = fields
                                            .iter()
                                            .find(|(k, _)| k == "event_time")
                                            .and_then(|(_, v)| match v {
                                                Value::Double(d) => Some(*d),
                                                Value::Float(f) => Some(*f as f64),
                                                _ => None,
                                            })
                                            .unwrap_or(0.0);

                                        let content = fields
                                            .iter()
                                            .find(|(k, _)| k == "content")
                                            .and_then(|(_, v)| match v {
                                                Value::String(s) => Some(s.clone()),
                                                _ => None,
                                            })
                                            .unwrap_or_else(|| "Unknown".to_string());

                                        let received_time = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .expect("Time went backwards")
                                            .as_secs_f64();
                                        let latency = received_time - event_time;

                                        tracing::info!(
                                            "📨 New message: [ID={}] {} at {:.3}",
                                            message_id,
                                            content,
                                            event_time
                                        );
                                        tracing::info!("⏱️ Latency: {:.3} seconds\n", latency);
                                    }
                                    Ok(other) => tracing::warn!("⚠️ Unexpected Avro value type: {:?}", other),
                                    Err(e) => tracing::warn!("⚠️ Avro deserialization error: {:?}", e),
                                }
                            }
                        }
                    }
                    Err(e) => tracing::error!("❌ Poll error: {:?}", e),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("🛑 Shutting down gracefully...");
                if let Err(e) = consumer.commit().await {
                    tracing::error!("Failed to commit offsets on shutdown: {:?}", e);
                }
                break;
            }
        }
    }
}
