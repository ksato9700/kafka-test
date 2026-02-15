package com.example.kafka;

import java.util.Properties;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.apache.kafka.clients.producer.Callback;
import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerConfig;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.clients.producer.RecordMetadata;
import org.apache.kafka.common.serialization.StringSerializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class Producer {

    private static final Logger logger = LoggerFactory.getLogger(
        Producer.class
    );

    public static void main(String[] args) {
        String bootstrapServers = System.getenv().getOrDefault(
            "KAFKA_BOOTSTRAP_SERVERS",
            "127.0.0.1:9094"
        );
        String topic = System.getenv().getOrDefault("TOPIC_NAME", "my-topic-1");

        logger.info("Hello from kafka-test-java!");
        logger.info(
            "🛠️ Connecting KafkaProducer to topic '{}' at '{}'...",
            topic,
            bootstrapServers
        );

        Properties props = new Properties();
        props.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        props.put(
            ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG,
            StringSerializer.class.getName()
        );
        props.put(
            ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG,
            StringSerializer.class.getName()
        );
        props.put(ProducerConfig.ACKS_CONFIG, "all");
        props.put(ProducerConfig.LINGER_MS_CONFIG, "0");

        // Force IPv4 for local dev
        System.setProperty("java.net.preferIPv4Stack", "true");

        KafkaProducer<String, String> producer = new KafkaProducer<>(props);

        final AtomicBoolean running = new AtomicBoolean(true);
        Runtime.getRuntime().addShutdownHook(
            new Thread(() -> {
                logger.info("🛑 Shutting down producer...");
                running.set(false);
                producer.close();
                logger.info("✅ Producer closed.");
            })
        );

        logger.info("🚀 Producer is now running...");

        long messageId = 0;
        try {
            while (running.get()) {
                double eventTime = System.currentTimeMillis() / 1000.0;
                String content = String.format(
                    "{\"message_id\":%d,\"event_time\":%.3f,\"content\":\"Message %d\"}",
                    messageId,
                    eventTime,
                    messageId
                );

                producer.send(
                    new ProducerRecord<>(topic, content),
                    new Callback() {
                        @Override
                        public void onCompletion(
                            RecordMetadata metadata,
                            Exception exception
                        ) {
                            if (exception != null) {
                                logger.error(
                                    "❌ Error sending message: ",
                                    exception
                                );
                            } else {
                                logger.info("🚀 Sent: {}", content);
                            }
                        }
                    }
                );

                messageId++;
                TimeUnit.SECONDS.sleep(2);
            }
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        } catch (Exception e) {
            logger.error("❌ Unexpected error: ", e);
        } finally {
            producer.close();
        }
    }
}
