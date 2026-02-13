use rdkafka::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use serde::Serialize;
use std::env;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time;

#[derive(Serialize)]
struct Message {
    message_id: u64,
    event_time: f64,
    content: String,
}

#[tokio::main]
async fn main() {
    println!("Hello from kafka-test-rust!");

    let bootstrap_servers = env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "localhost:9092".to_string());
    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    println!("🚀 Producer is now running...");

    let mut message_id = 0;
    loop {
        let event_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs_f64();

        let message = Message {
            message_id,
            event_time,
            content: format!("Message {}", message_id),
        };

        let payload = serde_json::to_string(&message).expect("Failed to serialize message");

        match producer
            .send(
                FutureRecord::<(), str>::to(&topic).payload(&payload),
                Timeout::Never,
            )
            .await
        {
            Ok(_) => println!("🚀 Sent: {:?}", payload),
            Err(e) => eprintln!("❌ Error sending message: {:?}", e),
        }

        message_id += 1;
        time::sleep(Duration::from_secs(2)).await;
    }
}
