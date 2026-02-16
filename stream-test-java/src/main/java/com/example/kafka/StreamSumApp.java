package com.example.kafka;

import org.apache.avro.Schema;
import org.apache.avro.generic.GenericData;
import org.apache.avro.generic.GenericDatumReader;
import org.apache.avro.generic.GenericDatumWriter;
import org.apache.avro.generic.GenericRecord;
import org.apache.avro.io.*;
import org.apache.kafka.clients.admin.AdminClient;
import org.apache.kafka.clients.admin.NewTopic;
import org.apache.kafka.common.serialization.Serde;
import org.apache.kafka.common.serialization.Serdes;
import org.apache.kafka.streams.KafkaStreams;
import org.apache.kafka.streams.StreamsBuilder;
import org.apache.kafka.streams.StreamsConfig;
import org.apache.kafka.streams.Topology;
import org.apache.kafka.streams.kstream.Consumed;
import org.apache.kafka.streams.kstream.KStream;
import org.apache.kafka.streams.kstream.Produced;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Properties;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicLong;

public class StreamSumApp {
    private static final Logger logger = LoggerFactory.getLogger(StreamSumApp.class);
    private static final AtomicLong messageCounter = new AtomicLong(0);

    // ThreadLocal to reuse Avro objects safely across stream threads
    private static class AvroContext {
        final ByteArrayOutputStream out = new ByteArrayOutputStream(1024);
        BinaryEncoder encoder = null;
        BinaryDecoder decoder = null;
        final GenericDatumWriter<GenericRecord> writer;
        final GenericDatumReader<GenericRecord> reader;

        AvroContext(Schema schema) {
            this.writer = new GenericDatumWriter<>(schema);
            this.reader = new GenericDatumReader<>(schema);
        }
    }

    private static void createTopics(String bootstrapServers, List<String> topics) {
        Properties props = new Properties();
        props.put(StreamsConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        try (AdminClient adminClient = AdminClient.create(props)) {
            for (String topic : topics) {
                NewTopic newTopic = new NewTopic(topic, 3, (short) 1);
                adminClient.createTopics(Collections.singletonList(newTopic)).all().get(2, TimeUnit.SECONDS);
            }
        } catch (Exception ignored) {}
    }

    private static Schema loadSchema(String fileName) {
        Path[] paths = { Paths.get("../schemas", fileName), Paths.get("schemas", fileName), Paths.get("/app/schemas", fileName) };
        for (Path p : paths) {
            if (Files.exists(p)) {
                try { return new Schema.Parser().parse(p.toFile()); }
                catch (IOException e) { throw new RuntimeException(e); }
            }
        }
        throw new RuntimeException("Schema file not found: " + fileName);
    }

    private static Serde<GenericRecord> createAvroSerde(Schema schema) {
        ThreadLocal<AvroContext> context = ThreadLocal.withInitial(() -> new AvroContext(schema));
        return Serdes.serdeFrom(
            (topic, data) -> {
                AvroContext ctx = context.get();
                ctx.out.reset();
                ctx.encoder = EncoderFactory.get().binaryEncoder(ctx.out, ctx.encoder);
                try {
                    ctx.writer.write(data, ctx.encoder);
                    ctx.encoder.flush();
                    return ctx.out.toByteArray();
                } catch (IOException e) { throw new RuntimeException(e); }
            },
            (topic, data) -> {
                AvroContext ctx = context.get();
                ctx.decoder = DecoderFactory.get().binaryDecoder(data, ctx.decoder);
                try { return ctx.reader.read(null, ctx.decoder); }
                catch (IOException e) { throw new RuntimeException(e); }
            }
        );
    }

    public static void main(String[] args) {
        Schema inputSchema = loadSchema("integer_list.avsc");
        Schema outputSchema = loadSchema("sum_result.avsc");

        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String inputTopic = System.getenv().getOrDefault("INPUT_TOPIC", "integer-list-input-avro");
        String outputTopic = System.getenv().getOrDefault("OUTPUT_TOPIC", "integer-sum-output-avro");
        String applicationId = System.getenv().getOrDefault("APPLICATION_ID", "stream-sum-test-java-avro");

        createTopics(bootstrapServers, Arrays.asList(inputTopic, outputTopic));

        Properties props = new Properties();
        props.put(StreamsConfig.APPLICATION_ID_CONFIG, applicationId);
        props.put(StreamsConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers);
        props.put(StreamsConfig.DEFAULT_KEY_SERDE_CLASS_CONFIG, Serdes.String().getClass());
        
        // Performance tweaks
        props.put(StreamsConfig.TOPOLOGY_OPTIMIZATION_CONFIG, StreamsConfig.OPTIMIZE);
        
        System.getenv().forEach((key, value) -> {
            if (key.startsWith("STREAMS_")) {
                String propName = key.substring(8).toLowerCase().replace("_", ".");
                props.put(propName, value);
                logger.info("Config: {} = {}", propName, value);
            }
        });

        StreamsBuilder builder = new StreamsBuilder();
        KStream<String, GenericRecord> source = builder.stream(inputTopic, 
            Consumed.with(Serdes.String(), createAvroSerde(inputSchema)));

        source.mapValues(record -> {
                @SuppressWarnings("unchecked")
                List<Integer> numbers = (List<Integer>) record.get("numbers");
                
                // Optimized sum: simple for loop to avoid stream overhead
                long sum = 0;
                for (Integer n : numbers) {
                    if (n != null) sum += n;
                }
                
                messageCounter.incrementAndGet();
                
                GenericRecord result = new GenericData.Record(outputSchema);
                result.put("sum", sum);
                return result;
            })
            .to(outputTopic, Produced.with(Serdes.String(), createAvroSerde(outputSchema)));

        KafkaStreams streams = new KafkaStreams(builder.build(), props);
        ScheduledExecutorService reporter = Executors.newSingleThreadScheduledExecutor();
        reporter.scheduleAtFixedRate(() -> {
            long count = messageCounter.getAndSet(0);
            if (count > 0) logger.info("🚀 Throughput: {} messages/sec", count / 5);
        }, 5, 5, TimeUnit.SECONDS);

        final CountDownLatch latch = new CountDownLatch(1);
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            reporter.shutdown();
            streams.close();
            latch.countDown();
        }));

        try {
            logger.info("Starting Optimized Avro Kafka Streams...");
            streams.start();
            latch.await();
        } catch (Throwable e) { System.exit(1); }
    }
}
