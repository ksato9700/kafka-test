use apache_avro::Schema;
use apache_avro::types::Record;
use krafka::producer::Producer;
use std::env;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Hello from kafka-test-krafka!");

    let schema = load_schema();
    tracing::info!("Loaded Avro schema");

    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());

    let producer = Producer::builder()
        .bootstrap_servers(&bootstrap_servers)
        .client_id("kafka-test-krafka-producer")
        .build()
        .await
        .expect("Failed to create producer");

    tracing::info!("🚀 Producer is now running...");

    let mut message_id: i64 = 0;
    loop {
        let event_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let mut record = Record::new(&schema).unwrap();
        record.put("message_id", message_id);
        record.put("event_time", event_time);
        record.put("content", format!("Message {}", message_id));

        let payload =
            apache_avro::to_avro_datum(&schema, record).expect("Failed to serialize to Avro");
        let n = payload.len();

        match producer.send(&topic, None, &payload).await {
            Ok(_) => tracing::info!("🚀 Sent: Message {} ({} bytes)", message_id, n),
            Err(e) => tracing::error!("❌ Error sending message: {:?}", e),
        }

        message_id += 1;

        tokio::select! {
            _ = time::sleep(Duration::from_secs(2)) => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("🛑 Shutting down producer...");
                producer.flush().await.expect("Failed to flush producer");
                break;
            }
        }
    }
}
