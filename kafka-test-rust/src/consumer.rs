use futures::TryStreamExt;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Message};
use serde::Deserialize;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize, Debug)]
struct KafkaMessage {
    message_id: u64,
    event_time: f64,
    content: String,
}

#[tokio::main]

async fn main() {
    let bootstrap_servers =
        env::var("KAFKA_BOOTSTRAP_SERVERS").unwrap_or_else(|_| "localhost:9092".to_string());

    let topic = env::var("TOPIC_NAME").unwrap_or_else(|_| "my-topic-1".to_string());

    let group_id = env::var("CONSUMER_GROUP_ID").unwrap_or_else(|_| "my-group-1".to_string());

    println!(
        "🛠️ Connecting KafkaConsumer to topic '{}' at '{}' (group: '{}')...",
        topic, bootstrap_servers, group_id
    );

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &bootstrap_servers)
        .set("group.id", &group_id)
        .set("enable.partition.eof", "false")
        .set("auto.offset.reset", "latest")
        .set("enable.auto.commit", "true")
        .set("broker.address.family", "v4")
        .create()
        .expect("Failed to create client");

    consumer
        .subscribe(&[&topic])
        .expect("Can't subscribe to specified topic");

    println!("✅ KafkaConsumer connected. Waiting for new messages...\n");
    println!("📥 Listening for messages...");

    let stream_processor = consumer.stream().try_for_each(|msg| async move {
        if let Some(payload) = msg.payload_view::<str>() {
            match payload {
                Ok(s) => match serde_json::from_str::<KafkaMessage>(s) {
                    Ok(data) => {
                        let received_time = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Time went backwards")
                            .as_secs_f64();
                        let latency = received_time - data.event_time;
                        println!(
                            "📨 New message: [ID={}] {} at {:.3}",
                            data.message_id, data.content, data.event_time
                        );
                        println!("⏱️ Latency: {:.3} seconds\n", latency);
                    }
                    Err(e) => eprintln!("⚠️ JSON parsing error: {:?} - payload: {}", e, s),
                },
                Err(e) => eprintln!("⚠️ Message payload is not a string: {:?}", e),
            }
        } else {
            println!("No payload");
        }
        Ok(())
    });

    tokio::select! {
        result = stream_processor => {
            if let Err(e) = result {
                eprintln!("❌ Stream processing error: {:?}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            println!("🛑 Shutting down gracefully...");
        }
    }
}
