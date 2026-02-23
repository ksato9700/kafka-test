package com.example.kafka;

import org.apache.kafka.clients.admin.AdminClient;
import org.apache.kafka.clients.admin.NewTopic;
import org.apache.kafka.clients.consumer.ConsumerConfig;
import org.apache.kafka.clients.consumer.ConsumerRecord;
import org.apache.kafka.clients.consumer.ConsumerRecords;
import org.apache.kafka.clients.consumer.KafkaConsumer;
import org.apache.kafka.clients.producer.KafkaProducer;
import org.apache.kafka.clients.producer.ProducerConfig;
import org.apache.kafka.clients.producer.ProducerRecord;
import org.apache.kafka.common.serialization.ByteArrayDeserializer;
import org.apache.kafka.common.serialization.ByteArraySerializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.time.Duration;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicLong;

public class StreamSumBenchmark {
    private static final Logger logger = LoggerFactory.getLogger(StreamSumBenchmark.class);
    private static final long TOTAL_RECORDS = 50_000_000L;
    private static final AtomicLong processedCounter = new AtomicLong(0);
    private static final AtomicLong startTimeNs = new AtomicLong(0);

    private static void createTopics(String bootstrapServers, List<String> topics) {
        Properties props = new Properties();
        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        try (AdminClient adminClient = AdminClient.create(props)) {
            for (String topic : topics) {
                NewTopic newTopic = new NewTopic(topic, 12, (short) 1);
                adminClient.createTopics(Collections.singletonList(newTopic)).all().get(5, TimeUnit.SECONDS);
            }
        } catch (Exception ignored) {}
    }

    private static void writeVarint(ByteArrayOutputStream out, long n) {
        long val = (n << 1) ^ (n >> 63);
        while (val >= 0x80) {
            out.write((int) (val | 0x80));
            val >>>= 7;
        }
        out.write((int) val);
    }

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

    public static void main(String[] args) throws Exception {
        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String inputTopic = "integer-list-input-benchmark";
        String outputTopic = "integer-sum-output-benchmark";
        String groupId = "benchmark-group-" + System.currentTimeMillis();
        int numWorkers = Integer.parseInt(System.getenv().getOrDefault("STREAMS_NUM_STREAM_THREADS", "8"));

        createTopics(bootstrapServers, Arrays.asList(inputTopic, outputTopic));

        // --- PHASE 1: LOAD ---
        logger.info("🚀 Phase 1: Loading {} records into Kafka...", TOTAL_RECORDS);
        Properties prodProps = new Properties();
        prodProps.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        prodProps.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
        prodProps.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
        prodProps.put(ProducerConfig.ACKS_CONFIG, "1");
        prodProps.put(ProducerConfig.BATCH_SIZE_CONFIG, "131072");
        prodProps.put(ProducerConfig.COMPRESSION_TYPE_CONFIG, "snappy");

        try (KafkaProducer<byte[], byte[]> producer = new KafkaProducer<>(prodProps)) {
            ByteArrayOutputStream out = new ByteArrayOutputStream(128);
            ThreadLocalRandom random = ThreadLocalRandom.current();
            for (long i = 0; i < TOTAL_RECORDS; i++) {
                out.reset();
                int count = random.nextInt(2, 6);
                writeVarint(out, count);
                for (int j = 0; j < count; j++) writeVarint(out, random.nextInt(0, 100));
                writeVarint(out, 0);
                producer.send(new ProducerRecord<>(inputTopic, out.toByteArray()));
            }
            producer.flush();
        }
        logger.info("✅ Phase 1 Complete.");

        // --- PHASE 2: PROCESS ---
        logger.info("🚀 Phase 2: Processing backlog with {} workers...", numWorkers);
        logger.info("Waiting for Kafka rebalance and first message...");
        CountDownLatch latch = new CountDownLatch(numWorkers);

        for (int i = 0; i < numWorkers; i++) {
            new Thread(() -> {
                KafkaProducer<byte[], byte[]> p = new KafkaProducer<>(prodProps);
                Properties consProps = new Properties();
                consProps.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
                consProps.put(ConsumerConfig.GROUP_ID_CONFIG, groupId);
                consProps.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
                consProps.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
                consProps.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "earliest");
                consProps.put(ConsumerConfig.FETCH_MIN_BYTES_CONFIG, "1000000");
                
                KafkaConsumer<byte[], byte[]> c = new KafkaConsumer<>(consProps);
                c.subscribe(Collections.singletonList(inputTopic));

                ByteArrayOutputStream outBuf = new ByteArrayOutputStream(64);
                try {
                    while (true) {
                        ConsumerRecords<byte[], byte[]> records = c.poll(Duration.ofMillis(100));
                        if (records.isEmpty() && processedCounter.get() >= TOTAL_RECORDS) break;
                        
                        for (ConsumerRecord<byte[], byte[]> record : records) {
                            // START TIMER ON FIRST RECORD
                            startTimeNs.compareAndSet(0, System.nanoTime());

                            byte[] payload = record.value();
                            if (payload == null) continue;
                            
                            ByteArrayInputStream in = new ByteArrayInputStream(payload);
                            long sum = 0;
                            long count = readVarint(in);
                            while (count != 0) {
                                long absCount = Math.abs(count);
                                if (count < 0) readVarint(in);
                                for (int j = 0; j < absCount; j++) sum += readVarint(in);
                                count = readVarint(in);
                            }
                            outBuf.reset();
                            writeVarint(outBuf, sum);
                            p.send(new ProducerRecord<>(outputTopic, outBuf.toByteArray()));
                            
                            if (processedCounter.incrementAndGet() >= TOTAL_RECORDS) return;
                        }
                    }
                } finally {
                    p.close();
                    c.close();
                    latch.countDown();
                }
            }).start();
        }

        // Progress Reporter
        Thread reporter = new Thread(() -> {
            while (processedCounter.get() < TOTAL_RECORDS) {
                try { Thread.sleep(1000); } catch (InterruptedException e) { break; }
                long current = processedCounter.get();
                if (current > 0) {
                    logger.info("Progress: {}% ({} / {})", (current * 100 / TOTAL_RECORDS), current, TOTAL_RECORDS);
                }
            }
        });
        reporter.setDaemon(true);
        reporter.start();

        latch.await();
        long endTimeNs = System.nanoTime();
        double totalTimeSeconds = (endTimeNs - startTimeNs.get()) / 1_000_000_000.0;

        logger.info("🏁 BENCHMARK RESULT (JAVA) 🏁");
        logger.info("Total Records: {}", TOTAL_RECORDS);
        logger.info("Actual Processing Time: {} seconds", String.format("%.2f", totalTimeSeconds));
        logger.info("Throughput:             {} messages/sec", String.format("%.0f", TOTAL_RECORDS / totalTimeSeconds));
        
        System.exit(0);
    }
}
