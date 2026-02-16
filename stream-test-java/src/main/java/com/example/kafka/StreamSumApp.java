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
import org.apache.kafka.common.serialization.StringDeserializer;
import org.apache.kafka.common.serialization.StringSerializer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Properties;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

public class StreamSumApp {
    private static final Logger logger = LoggerFactory.getLogger(StreamSumApp.class);
    private static final AtomicLong messageCounter = new AtomicLong(0);

    private static void createTopics(String bootstrapServers, List<String> topics) {
        Properties props = new Properties();
        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        try (AdminClient adminClient = AdminClient.create(props)) {
            for (String topic : topics) {
                NewTopic newTopic = new NewTopic(topic, 12, (short) 1);
                adminClient.createTopics(Collections.singletonList(newTopic)).all().get(2, TimeUnit.SECONDS);
            }
        } catch (Exception ignored) {}
    }

    // High-performance Zig-Zag Decoding
    private static long readVarint(ByteArrayInputStream in) {
        long val = 0;
        int shift = 0;
        while (true) {
            int b = in.read();
            val |= (long) (b & 0x7F) << shift;
            if ((b & 0x80) == 0) break;
            shift += 7;
        }
        return (val >>> 1) ^ -(val & 1);
    }

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
        String inputTopic = System.getenv().getOrDefault("INPUT_TOPIC", "integer-list-input-avro");
        String outputTopic = System.getenv().getOrDefault("OUTPUT_TOPIC", "integer-sum-output-avro");
        String groupId = System.getenv().getOrDefault("APPLICATION_ID", "stream-sum-test-java-avro");
        int numWorkers = Integer.parseInt(System.getenv().getOrDefault("STREAMS_NUM_STREAM_THREADS", "3"));

        createTopics(bootstrapServers, Arrays.asList(inputTopic, outputTopic));

        // Throughput Reporter
        ScheduledExecutorService reporter = Executors.newSingleThreadScheduledExecutor();
        reporter.scheduleAtFixedRate(() -> {
            long count = messageCounter.getAndSet(0);
            if (count > 0) {
                logger.info("🚀 Java Throughput: {} messages/sec", count / 5);
            }
        }, 5, 5, TimeUnit.SECONDS);

        logger.info("🚀 Starting Ultra-Performance Java Workers: {}", numWorkers);

        for (int i = 0; i < numWorkers; i++) {
            final int workerId = i;
            new Thread(() -> {
                // Per-thread Producer
                Properties prodProps = new Properties();
                prodProps.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
                prodProps.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
                prodProps.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, ByteArraySerializer.class.getName());
                prodProps.put(ProducerConfig.ACKS_CONFIG, "1");
                prodProps.put(ProducerConfig.BATCH_SIZE_CONFIG, "131072");
                prodProps.put(ProducerConfig.LINGER_MS_CONFIG, "20");
                prodProps.put(ProducerConfig.COMPRESSION_TYPE_CONFIG, "snappy");
                KafkaProducer<byte[], byte[]> producer = new KafkaProducer<>(prodProps);

                // Per-thread Consumer
                Properties consProps = new Properties();
                consProps.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
                consProps.put(ConsumerConfig.GROUP_ID_CONFIG, groupId);
                consProps.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
                consProps.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, ByteArrayDeserializer.class.getName());
                consProps.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "earliest");
                consProps.put(ConsumerConfig.FETCH_MIN_BYTES_CONFIG, "100000");
                consProps.put(ConsumerConfig.FETCH_MAX_WAIT_MS_CONFIG, "100");
                KafkaConsumer<byte[], byte[]> consumer = new KafkaConsumer<>(consProps);
                consumer.subscribe(Collections.singletonList(inputTopic));

                ByteArrayOutputStream outBuf = new ByteArrayOutputStream(64);
                long localCounter = 0;

                logger.info("Worker thread {} started", workerId);

                try {
                    while (true) {
                        ConsumerRecords<byte[], byte[]> records = consumer.poll(Duration.ofMillis(10));
                        for (ConsumerRecord<byte[], byte[]> record : records) {
                            byte[] payload = record.value();
                            if (payload == null) continue;

                            ByteArrayInputStream in = new ByteArrayInputStream(payload);
                            long sum = 0;
                            long count = readVarint(in);
                            while (count != 0) {
                                long absCount = Math.abs(count);
                                if (count < 0) readVarint(in); // skip byte size
                                for (int j = 0; j < absCount; j++) {
                                    sum += readVarint(in);
                                }
                                count = readVarint(in);
                            }

                            outBuf.reset();
                            writeVarint(outBuf, sum);

                            producer.send(new ProducerRecord<>(outputTopic, record.key(), outBuf.toByteArray()));
                            
                            localCounter++;
                            if (localCounter >= 1000) {
                                messageCounter.addAndGet(localCounter);
                                localCounter = 0;
                            }
                        }
                    }
                } catch (Exception e) {
                    logger.error("Worker {} failed", workerId, e);
                } finally {
                    producer.close();
                    consumer.close();
                }
            }).start();
        }
    }
}
