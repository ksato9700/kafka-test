package com.example.kafka;

import org.apache.kafka.clients.admin.AdminClient;
import org.apache.kafka.clients.admin.ListConsumerGroupOffsetsResult;
import org.apache.kafka.clients.admin.OffsetSpec;
import org.apache.kafka.clients.consumer.OffsetAndMetadata;
import org.apache.kafka.common.TopicPartition;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Collections;
import java.util.Map;
import java.util.Properties;
import java.util.concurrent.TimeUnit;
import java.util.stream.Collectors;

public class KafkaLagMonitor {
    private static final Logger logger = LoggerFactory.getLogger(KafkaLagMonitor.class);

    public static void main(String[] args) throws Exception {
        String bootstrapServers = System.getenv().getOrDefault("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094");
        String groupId = args.length > 0 && !args[0].isEmpty() ? args[0] : "stream-sum-test-java-avro";

        Properties props = new Properties();
        props.put("bootstrap.servers", bootstrapServers);

        try (AdminClient adminClient = AdminClient.create(props)) {
            logger.info("Monitoring lag for group: {}", groupId);
            while (true) {
                try {
                    ListConsumerGroupOffsetsResult offsetsResult = adminClient.listConsumerGroupOffsets(groupId);
                    Map<TopicPartition, OffsetAndMetadata> groupOffsets = offsetsResult.partitionsToOffsetAndMetadata().get(5, TimeUnit.SECONDS);

                    if (groupOffsets.isEmpty()) {
                        System.out.print("\rGroup '" + groupId + "' not found or has no offsets...          ");
                    } else {
                        Map<TopicPartition, OffsetSpec> latestOffsetSpecs = groupOffsets.keySet().stream()
                                .collect(Collectors.toMap(tp -> tp, tp -> OffsetSpec.latest()));
                        
                        var endOffsets = adminClient.listOffsets(latestOffsetSpecs).all().get(5, TimeUnit.SECONDS);

                        long totalLag = 0;
                        for (Map.Entry<TopicPartition, OffsetAndMetadata> entry : groupOffsets.entrySet()) {
                            TopicPartition tp = entry.getKey();
                            long currentOffset = entry.getValue().offset();
                            long endOffset = endOffsets.get(tp).offset();
                            totalLag += Math.max(0, endOffset - currentOffset);
                        }

                        System.out.print("\rGroup: " + groupId + " | Total Lag: " + totalLag + " messages          ");
                    }
                } catch (Exception e) {
                    System.out.print("\rError fetching offsets: " + e.getMessage() + "          ");
                }
                
                TimeUnit.SECONDS.sleep(2);
            }
        }
    }
}
