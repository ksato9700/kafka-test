package com.example.kafka;

import org.apache.avro.Schema;
import org.apache.avro.generic.GenericData;
import org.apache.avro.generic.GenericDatumWriter;
import org.apache.avro.generic.GenericRecord;
import org.apache.avro.io.BinaryEncoder;
import org.apache.avro.io.EncoderFactory;
import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerConfig;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.common.serialization.ByteArraySerializer;
import org.apache.kafka.common.serialization.StringSerializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import java.util.Random;
import java.util.concurrent.atomic.AtomicLong;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;

public class TestProducer {
    private static final Logger logger = LoggerFactory.getLogger(TestProducer.class);
    private static final Random random = new Random();
    private static final AtomicLong sendCounter = new AtomicLong(0);

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
        Schema schema = loadSchema("integer_list.avsc");
        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String topic = System.getenv().getOrDefault("INPUT_TOPIC", "integer-list-input-avro");

        Properties props = new Properties();
        props.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        props.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, StringSerializer.class.getName());
        props.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
        
        // Performance Tuning
        props.put(ProducerConfig.ACKS_CONFIG, "1"); // Wait only for leader
        props.put(ProducerConfig.BATCH_SIZE_CONFIG, "65536");
        props.put(ProducerConfig.LINGER_MS_CONFIG, "20");
        props.put(ProducerConfig.COMPRESSION_TYPE_CONFIG, "snappy");
        props.put(ProducerConfig.BUFFER_MEMORY_CONFIG, "33554432");

        KafkaProducer<String, byte[]> producer = new KafkaProducer<>(props);
        GenericDatumWriter<GenericRecord> writer = new GenericDatumWriter<>(schema);

        // Throughput Reporter
        ScheduledExecutorService reporter = Executors.newSingleThreadScheduledExecutor();
        reporter.scheduleAtFixedRate(() -> {
            long count = sendCounter.getAndSet(0);
            if (count > 0) {
                logger.info("📤 Sending Throughput: {} messages/sec", count / 5);
            }
        }, 5, 5, TimeUnit.SECONDS);

        logger.info("🚀 Starting High-Performance Avro Producer on topic: {}", topic);

        try {
            while (true) {
                List<Integer> numbers = new ArrayList<>();
                int count = random.nextInt(4) + 2;
                for (int i = 0; i < count; i++) numbers.add(random.nextInt(100));

                GenericRecord record = new GenericData.Record(schema);
                record.put("numbers", numbers);

                ByteArrayOutputStream out = new ByteArrayOutputStream();
                BinaryEncoder encoder = EncoderFactory.get().binaryEncoder(out, null);
                writer.write(record, encoder);
                encoder.flush();

                producer.send(new ProducerRecord<>(topic, null, out.toByteArray()));
                sendCounter.incrementAndGet();
            }
        } catch (Exception e) {
            logger.error("Producer error", e);
        } finally {
            reporter.shutdown();
            producer.close();
        }
    }
}
