use apache_avro::Schema;
use apache_avro::types::Value;
use futures::TryStreamExt;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn load_schema() -> Schema {
    let paths = [
        "../schemas/message.avsc",
        "/app/schemas/message.avsc",
        "schemas/message.avsc",
    ];

    for p in paths {
        if Path::new(p).exists() {
            let mut file = File::open(p).expect("Failed to open schema file");
            let mut content = String::new();
            file.read_to_string(&mut content)
                .expect("Failed to read schema file");
            return Schema::parse_str(&content).expect("Failed to parse schema");
        }
    }
    panic!("Schema file not found in paths: {:?}", paths);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let schema = Arc::new(load_schema());
    tracing::info!("Loaded Avro schema");

    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());

    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());

    let group_id = env::var("CONSUMER_GROUP_ID").unwrap_or_else(|_| "my-group-1".to_string());

    let auto_offset_reset = env::var("AUTO_OFFSET_RESET").unwrap_or_else(|_| "latest".to_string());

    tracing::info!(
        "🛠️ Connecting KafkaConsumer to topic '{}' at '{}' (group: '{}')...",
        topic,
        bootstrap_servers,
        group_id
    );

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("group.id", &group_id)
        .set("enable.partition.eof", "false")
        .set("auto.offset.reset", &auto_offset_reset)
        .set("enable.auto.commit", "true")
        .set("broker.address.family", "v4")
        .create()
        .expect("Failed to create client");

    consumer
        .subscribe(&[&topic])
        .expect("Can't subscribe to specified topic");

    tracing::info!("✅ KafkaConsumer initialized. Verifying connectivity...");

    match consumer.fetch_metadata(Some(&topic), std::time::Duration::from_secs(5)) {
        Ok(metadata) => {
            tracing::info!(
                "✅ Successfully connected to broker. Metadata: {:?}",
                metadata.topics()
            );
        }
        Err(e) => {
            tracing::error!("❌ Failed to fetch metadata from broker: {:?}", e);
        }
    }

    tracing::info!("📥 Listening for messages...");

    let stream_processor = consumer.stream().try_for_each(|msg| {
        let schema = schema.clone();
        async move {
            if let Some(payload) = msg.payload() {
                let mut reader = &payload[..];
                match apache_avro::from_avro_datum(&schema, &mut reader, Some(&schema)) {
                    Ok(value) => {
                        if let Value::Record(fields) = value {
                            // Extract fields manually
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
                        } else {
                            tracing::warn!("⚠️ Unexpected Avro value type: {:?}", value);
                        }
                    }
                    Err(e) => tracing::warn!("⚠️ Avro deserialization error: {:?}", e),
                }
            } else {
                tracing::info!("No payload");
            }
            Ok(())
        }
    });

    tokio::select! {
        result = stream_processor => {
            if let Err(e) = result {
                tracing::error!("❌ Stream processing error: {:?}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("🛑 Shutting down gracefully...");
        }
    }
}
