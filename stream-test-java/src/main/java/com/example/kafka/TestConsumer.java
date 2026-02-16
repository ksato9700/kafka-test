package com.example.kafka;

import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.ConsumerRecords;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayInputStream;
import java.time.Duration;
import java.util.Collections;
import java.util.Properties;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

public class TestConsumer {
    private static final Logger logger = LoggerFactory.getLogger(TestConsumer.class);
    private static final AtomicLong resultCounter = new AtomicLong(0);

    // High-performance Zig-Zag Decoding
    private static long readVarint(ByteArrayInputStream in) {
        long val = 0;
        int shift = 0;
        while (true) {
            int b = in.read();
            if (b == -1) return 0;
            val |= (long) (b & 0x7F) << shift;
            if ((b & 0x80) == 0) break;
            shift += 7;
        }
        return (val >>> 1) ^ -(val & 1);
    }

    public static void main(String[] args) {
        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String topic = System.getenv().getOrDefault("OUTPUT_TOPIC", "integer-sum-output-avro");
        int numThreads = Integer.parseInt(System.getenv().getOrDefault("CONSUMER_THREADS", "1"));

        // Throughput Reporter
        ScheduledExecutorService reporter = Executors.newSingleThreadScheduledExecutor();
        reporter.scheduleAtFixedRate(() -> {
            long count = resultCounter.getAndSet(0);
            if (count > 0) {
                logger.info("📥 Java Result Throughput: {} messages/sec", count / 5);
            }
        }, 5, 5, TimeUnit.SECONDS);

        logger.info("🚀 Starting Ultra-Performance Java Consumers: {}", numThreads);

        for (int i = 0; i < numThreads; i++) {
            final int threadId = i;
            new Thread(() -> {
                Properties props = new Properties();
                props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
                props.put(ConsumerConfig.GROUP_ID_CONFIG, "stream-test-result-consumer-" + threadId);
                props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
                props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
                props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "latest");
                props.put(ConsumerConfig.FETCH_MIN_BYTES_CONFIG, "100000");

                KafkaConsumer<byte[], byte[]> consumer = new KafkaConsumer<>(props);
                consumer.subscribe(Collections.singletonList(topic));

                logger.info("Consumer thread {} started", threadId);

                try {
                    while (true) {
                        ConsumerRecords<byte[], byte[]> records = consumer.poll(Duration.ofMillis(100));
                        for (ConsumerRecord<byte[], byte[]> record : records) {
                            byte[] payload = record.value();
                            if (payload != null) {
                                ByteArrayInputStream in = new ByteArrayInputStream(payload);
                                long sum = readVarint(in);
                                // For performance testing, we don't log every message
                                resultCounter.incrementAndGet();
                            }
                        }
                    }
                } catch (Exception e) {
                    logger.error("Consumer thread {} failed", threadId, e);
                } finally {
                    consumer.close();
                }
            }).start();
        }
    }
}
