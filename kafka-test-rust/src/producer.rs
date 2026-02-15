use apache_avro::Schema;
use apache_avro::types::Record;
use rdkafka::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;

fn load_schema() -> Schema {
    let paths = [
        "../schemas/message.avsc",
        "/app/schemas/message.avsc",
        "schemas/message.avsc", // Fallback
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
    tracing::info!("Hello from kafka-test-rust!");

    let schema = load_schema();
    tracing::info!("Loaded Avro schema");

    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "127.0.0.1:9094".to_string());
    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("message.timeout.ms", "5000")
        .set("broker.address.family", "v4")
        .create()
        .expect("Producer creation error");

    tracing::info!("🚀 Producer is now running...");

    let mut message_id = 0;
    loop {
        let event_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        // Create Avro record manually since we don't have generated structs
        // Alternatively we could define a struct with Serde and use to_avro_datum
        let mut record = Record::new(&schema).unwrap();
        record.put("message_id", message_id);
        record.put("event_time", event_time);
        record.put("content", format!("Message {}", message_id));

        let payload =
            apache_avro::to_avro_datum(&schema, record).expect("Failed to serialize to Avro");

        // Send message
        match producer
            .send(
                FutureRecord::<(), [u8]>::to(&topic).payload(&payload),
                Timeout::Never,
            )
            .await
        {
            Ok(_) => tracing::info!("🚀 Sent: Message {} ({} bytes)", message_id, payload.len()),
            Err(e) => tracing::error!("❌ Error sending message: {:?}", e),
        }

        message_id += 1;

        // Wait with cancellation support
        tokio::select! {
            _ = time::sleep(Duration::from_secs(2)) => {}
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("🛑 Shutting down producer...");
                break;
            }
        }
    }
}
