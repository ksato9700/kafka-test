package com.example.kafka;

import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.util.Collections;
import java.util.Properties;
import java.util.concurrent.atomic.AtomicBoolean;
import org.apache.avro.Schema;
import org.apache.avro.generic.GenericDatumReader;
import org.apache.avro.generic.GenericRecord;
import org.apache.avro.io.BinaryDecoder;
import org.apache.avro.io.DecoderFactory;
import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.ConsumerRecords;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.common.errors.WakeupException;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.StringDeserializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class Consumer {

    private static final Logger logger = LoggerFactory.getLogger(
        Consumer.class
    );

    private static Schema loadSchema() throws Exception {
        Path[] paths = {
            Paths.get("../schemas/message.avsc"),
            Paths.get("/app/schemas/message.avsc"),
            Paths.get("schemas/message.avsc"),
        };

        for (Path p : paths) {
            if (Files.exists(p)) {
                return new Schema.Parser().parse(p.toFile());
            }
        }
        throw new RuntimeException("Schema file not found in paths");
    }

    public static void main(String[] args) {
        try {
            Schema schema = loadSchema();
            logger.info("Loaded Avro schema: {}", schema.getFullName());

            String bootstrapServers = System.getenv().getOrDefault(
                "KAFKA_BOOTSTRAP_SERVERS",
                "127.0.0.1:9094"
            );
            String topic = System.getenv().getOrDefault(
                "TOPIC_NAME",
                "my-topic-1"
            );
            String groupId = System.getenv().getOrDefault(
                "CONSUMER_GROUP_ID",
                "my-group-1"
            );
            String offsetReset = System.getenv().getOrDefault(
                "AUTO_OFFSET_RESET",
                "latest"
            );

            logger.info(
                "🛠️ Connecting KafkaConsumer to topic '{}' at '{}' (group: '{}')...",
                topic,
                bootstrapServers,
                groupId
            );

            Properties props = new Properties();
            props.put(
                ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG,
                bootstrapServers
            );
            props.put(ConsumerConfig.GROUP_ID_CONFIG, groupId);
            props.put(
                ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG,
                StringDeserializer.class.getName()
            );
            props.put(
                ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG,
                ByteArrayDeserializer.class.getName()
            );
            props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, offsetReset);
            props.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, "true");
            props.put(ConsumerConfig.SESSION_TIMEOUT_MS_CONFIG, "6000"); // Fail fast if consumer dies

            // Use IPv4 if possible to avoid connection issues on localhost
            System.setProperty("java.net.preferIPv4Stack", "true");

            KafkaConsumer<String, byte[]> consumer = new KafkaConsumer<>(props);

            // Graceful shutdown
            final AtomicBoolean closed = new AtomicBoolean(false);
            Runtime.getRuntime().addShutdownHook(
                new Thread(() -> {
                    logger.info("🛑 Shutting down gracefully...");
                    closed.set(true);
                    consumer.wakeup();
                })
            );

            GenericDatumReader<GenericRecord> reader = new GenericDatumReader<>(
                schema
            );

            try {
                consumer.subscribe(Collections.singletonList(topic));
                logger.info(
                    "✅ KafkaConsumer connected. Waiting for new messages..."
                );
                logger.info("📥 Listening for messages...");

                while (!closed.get()) {
                    ConsumerRecords<String, byte[]> records = consumer.poll(
                        Duration.ofMillis(100)
                    );
                    for (ConsumerRecord<String, byte[]> record : records) {
                        try {
                            BinaryDecoder decoder =
                                DecoderFactory.get().binaryDecoder(
                                    record.value(),
                                    null
                                );
                            GenericRecord avroRecord = reader.read(
                                null,
                                decoder
                            );

                            long messageId = (long) avroRecord.get(
                                "message_id"
                            );
                            double eventTime = (double) avroRecord.get(
                                "event_time"
                            );
                            String content = avroRecord
                                .get("content")
                                .toString();

                            long nowMillis = System.currentTimeMillis();
                            double nowSeconds = nowMillis / 1000.0;
                            double latency = nowSeconds - eventTime;

                            logger.info(
                                "📨 New message: [ID={}] {} at {}",
                                messageId,
                                content,
                                String.format("%.3f", eventTime)
                            );
                            logger.info(
                                "⏱️ Latency: {} seconds",
                                String.format("%.3f", latency)
                            );
                        } catch (Exception e) {
                            logger.warn(
                                "⚠️ Error parsing message: {} - offset: {}",
                                e.getMessage(),
                                record.offset()
                            );
                        }
                    }
                }
            } catch (WakeupException e) {
                if (!closed.get()) throw e;
            } catch (Exception e) {
                logger.error("❌ KafkaConsumer error: ", e);
            } finally {
                consumer.close();
                logger.info("✅ KafkaConsumer closed.");
            }
        } catch (Exception e) {
            logger.error("❌ Fatal error: ", e);
        }
    }
}
