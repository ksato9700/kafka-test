package main

import (
	"fmt"
	"os"

	"github.com/confluentinc/confluent-kafka-go/v2/kafka"
)

func readVarint(data []byte, pos *int) int64 {
	var val uint64
	var shift uint
	for {
		b := data[*pos]
		*pos++
		val |= uint64(b&0x7F) << shift
		if b&0x80 == 0 {
			break
		}
		shift += 7
	}
	return int64(val>>1) ^ -int64(val&1)
}

func main() {
	bootstrapServers := getEnv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
	topic := getEnv("OUTPUT_TOPIC", "integer-sum-output-avro")

	consumer, err := kafka.NewConsumer(&kafka.ConfigMap{
		"bootstrap.servers":     bootstrapServers,
		"broker.address.family": "v4",
		"group.id":              "go-test-result-consumer",
		"auto.offset.reset":     "earliest",
	})
	if err != nil {
		panic(err)
	}
	defer consumer.Close()

	consumer.Subscribe(topic, nil)
	fmt.Printf("🚀 Waiting for Avro results on topic: %s...\n", topic)

	for {
		ev := consumer.Poll(100)
		if ev == nil {
			continue
		}

		switch e := ev.(type) {
		case *kafka.Message:
			pos := 0
			sum := readVarint(e.Value, &pos)
			fmt.Printf("Received Avro Result: sum=%d\n", sum)
		case kafka.Error:
			fmt.Fprintf(os.Stderr, "%% Error: %v\n", e)
		}
	}
}

func getEnv(key, fallback string) string {
	if value, ok := os.LookupEnv(key); ok {
		return value
	}
	return fallback
}
