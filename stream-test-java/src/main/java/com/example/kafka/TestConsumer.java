package com.example.kafka;

import org.apache.avro.Schema;
import org.apache.avro.generic.GenericDatumReader;
import org.apache.avro.generic.GenericRecord;
import org.apache.avro.io.BinaryDecoder;
import org.apache.avro.io.DecoderFactory;
import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.ConsumerRecords;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.StringDeserializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Duration;
import java.util.Collections;
import java.util.Properties;

public class TestConsumer {
    private static final Logger logger = LoggerFactory.getLogger(TestConsumer.class);

    private static Schema loadSchema(String fileName) {
        Path[] paths = { Paths.get("../schemas", fileName), Paths.get("schemas", fileName) };
        for (Path p : paths) {
            if (Files.exists(p)) {
                try {
                    return new Schema.Parser().parse(p.toFile());
                } catch (IOException e) {
                    throw new RuntimeException(e);
                }
            }
        }
        throw new RuntimeException("Schema not found: " + fileName);
    }

    public static void main(String[] args) {
        Schema schema = loadSchema("sum_result.avsc");
        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String topic = System.getenv().getOrDefault("OUTPUT_TOPIC", "integer-sum-output-avro");

        Properties props = new Properties();
        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        props.put(ConsumerConfig.GROUP_ID_CONFIG, "stream-test-consumer-group-avro");
        props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, StringDeserializer.class.getName());
        props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
        props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "earliest");

        KafkaConsumer<String, byte[]> consumer = new KafkaConsumer<>(props);
        consumer.subscribe(Collections.singletonList(topic));

        GenericDatumReader<GenericRecord> reader = new GenericDatumReader<>(schema);

        logger.info("🚀 Waiting for Avro results on topic: {}", topic);

        try {
            while (true) {
                ConsumerRecords<String, byte[]> records = consumer.poll(Duration.ofMillis(100));
                for (ConsumerRecord<String, byte[]> record : records) {
                    BinaryDecoder decoder = DecoderFactory.get().binaryDecoder(record.value(), null);
                    GenericRecord result = reader.read(null, decoder);
                    logger.info("Received Avro Result: sum={}", result.get("sum"));
                }
            }
        } catch (Exception e) {
            logger.error("Consumer error", e);
        } finally {
            consumer.close();
        }
    }
}
