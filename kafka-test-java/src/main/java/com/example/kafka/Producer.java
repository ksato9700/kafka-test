package com.example.kafka;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Properties;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;
import org.apache.avro.Schema;
import org.apache.avro.generic.GenericData;
import org.apache.avro.generic.GenericDatumWriter;
import org.apache.avro.generic.GenericRecord;
import org.apache.avro.io.BinaryEncoder;
import org.apache.avro.io.EncoderFactory;
import org.apache.kafka.clients.producer.Callback;
import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerConfig;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.clients.producer.RecordMetadata;
import org.apache.kafka.common.serialization.ByteArraySerializer;
import org.apache.kafka.common.serialization.StringSerializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class Producer {

    private static final Logger logger = LoggerFactory.getLogger(
        Producer.class
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

            logger.info("Hello from kafka-test-java!");
            logger.info(
                "🛠️ Connecting KafkaProducer to topic '{}' at '{}'...",
                topic,
                bootstrapServers
            );

            Properties props = new Properties();
            props.put(
                ProducerConfig.BOOTSTRAP_SERVERS_CONFIG,
                bootstrapServers
            );
            props.put(
                ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG,
                StringSerializer.class.getName()
            );
            props.put(
                ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG,
                ByteArraySerializer.class.getName()
            );
            props.put(ProducerConfig.ACKS_CONFIG, "all");
            props.put(ProducerConfig.LINGER_MS_CONFIG, "0");

            // Force IPv4 for local dev
            System.setProperty("java.net.preferIPv4Stack", "true");

            KafkaProducer<String, byte[]> producer = new KafkaProducer<>(props);

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
            GenericDatumWriter<GenericRecord> writer = new GenericDatumWriter<>(
                schema
            );

            while (running.get()) {
                double eventTime = System.currentTimeMillis() / 1000.0;
                String contentStr = String.format("Message %d", messageId);

                GenericRecord avroRecord = new GenericData.Record(schema);
                avroRecord.put("message_id", messageId);
                avroRecord.put("event_time", eventTime);
                avroRecord.put("content", contentStr);

                ByteArrayOutputStream out = new ByteArrayOutputStream();
                BinaryEncoder encoder = EncoderFactory.get().binaryEncoder(
                    out,
                    null
                );
                writer.write(avroRecord, encoder);
                encoder.flush();
                byte[] payload = out.toByteArray();

                producer.send(
                    new ProducerRecord<>(topic, payload),
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
                                logger.info("🚀 Sent: {}", avroRecord);
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
        }
    }
}
