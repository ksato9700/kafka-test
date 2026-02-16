package com.example.kafka;

import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerConfig;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.common.serialization.ByteArraySerializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayOutputStream;
import java.util.Properties;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ThreadLocalRandom;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

public class TestProducer {
    private static final Logger logger = LoggerFactory.getLogger(TestProducer.class);
    private static final AtomicLong sendCounter = new AtomicLong(0);

    // High-performance Zig-Zag Encoding
    private static void writeVarint(ByteArrayOutputStream out, long n) {
        long val = (n << 1) ^ (n >> 63);
        while (val >= 0x80) {
            out.write((int) (val | 0x80));
            val >>>= 7;
        }
        out.write((int) val);
    }

    public static void main(String[] args) {
        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String topic = System.getenv().getOrDefault("INPUT_TOPIC", "integer-list-input-avro");
        int numThreads = Integer.parseInt(System.getenv().getOrDefault("PRODUCER_THREADS", "3"));

        // Throughput Reporter
        ScheduledExecutorService reporter = Executors.newSingleThreadScheduledExecutor();
        reporter.scheduleAtFixedRate(() -> {
            long count = sendCounter.getAndSet(0);
            if (count > 0) {
                logger.info("📤 Java Sending Throughput: {} messages/sec", count / 5);
            }
        }, 5, 5, TimeUnit.SECONDS);

        logger.info("🚀 Starting Ultra-Performance Java Producers: {}", numThreads);

        for (int i = 0; i < numThreads; i++) {
            final int threadId = i;
            new Thread(() -> {
                Properties props = new Properties();
                props.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
                props.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
                props.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
                props.put(ProducerConfig.ACKS_CONFIG, "1");
                props.put(ProducerConfig.BATCH_SIZE_CONFIG, "131072");
                props.put(ProducerConfig.LINGER_MS_CONFIG, "20");
                props.put(ProducerConfig.COMPRESSION_TYPE_CONFIG, "snappy");
                props.put(ProducerConfig.BUFFER_MEMORY_CONFIG, "67108864");

                KafkaProducer<byte[], byte[]> producer = new KafkaProducer<>(props);
                ByteArrayOutputStream out = new ByteArrayOutputStream(128);
                ThreadLocalRandom random = ThreadLocalRandom.current();

                logger.info("Producer thread {} started", threadId);

                try {
                    while (true) {
                        out.reset();
                        int count = random.nextInt(2, 6);
                        
                        // Avro Array: count (varint), then elements, then 0 (varint)
                        writeVarint(out, count);
                        for (int j = 0; j < count; j++) {
                            writeVarint(out, random.nextInt(0, 100));
                        }
                        writeVarint(out, 0); // End of array

                        producer.send(new ProducerRecord<>(topic, null, out.toByteArray()));
                        
                        long current = sendCounter.incrementAndGet();
                        // Optional: rate limit or yield if needed, but for max perf we just loop
                    }
                } catch (Exception e) {
                    logger.error("Producer thread {} failed", threadId, e);
                } finally {
                    producer.close();
                }
            }).start();
        }
    }
}
