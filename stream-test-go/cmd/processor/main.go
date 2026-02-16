package main

import (
	"fmt"
	"os"
	"os/signal"
	"strconv"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/confluentinc/confluent-kafka-go/v2/kafka"
)

var messageCounter uint64

// readVarint decodes a zig-zag encoded varint from a byte slice
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

// writeVarint encodes a zig-zag encoded varint into a byte slice
func writeVarint(buf []byte, n int64) []byte {
	val := uint64((n << 1) ^ (n >> 63))
	for val >= 0x80 {
		buf = append(buf, uint8(val)|0x80)
		val >>= 7
	}
	return append(buf, uint8(val))
}

func main() {
	bootstrapServers := getEnv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
	inputTopic := getEnv("INPUT_TOPIC", "integer-list-input-avro")
	outputTopic := getEnv("OUTPUT_TOPIC", "integer-sum-output-avro")
	groupID := getEnv("APPLICATION_ID", "stream-sum-test-go-avro")
	numWorkers, _ := strconv.Atoi(getEnv("NUM_WORKERS", "3"))

	// Throughput reporter
	go func() {
		ticker := time.NewTicker(5 * time.Second)
		for range ticker.C {
			count := atomic.SwapUint64(&messageCounter, 0)
			if count > 0 {
				fmt.Printf("🚀 Go Throughput: %d messages/sec\n", count/5)
			}
		}
	}()

	fmt.Printf("🚀 Starting High-Performance Go Stream Processor with %d workers...\n", numWorkers)

	sigchan := make(chan os.Signal, 1)
	signal.Notify(sigchan, syscall.SIGINT, syscall.SIGTERM)

	for i := 0; i < numWorkers; i++ {
		go startWorker(i, bootstrapServers, inputTopic, outputTopic, groupID)
	}

	<-sigchan
	fmt.Println("Shutting down...")
}

func startWorker(id int, bootstrap, inputTopic, outputTopic, groupID string) {
	config := &kafka.ConfigMap{
		"bootstrap.servers":        bootstrap,
		"broker.address.family":    "v4",
		"group.id":                 groupID,
		"auto.offset.reset":        "earliest",
		"enable.auto.commit":       true,
		"fetch.min.bytes":          100000,
		"go.events.channel.enable": false,
	}

	consumer, err := kafka.NewConsumer(config)
	if err != nil {
		panic(err)
	}
	defer consumer.Close()

	producer, err := kafka.NewProducer(&kafka.ConfigMap{
		"bootstrap.servers":     bootstrap,
		"broker.address.family": "v4",
		"compression.type":      "snappy",
		"linger.ms":             20,
		"batch.num.messages":    100000,
	})
	if err != nil {
		panic(err)
	}
	defer producer.Close()

	consumer.Subscribe(inputTopic, nil)

	fmt.Printf("Worker %d started\n", id)
	var localCounter uint64
	outBuf := make([]byte, 0, 64)

	for {
		ev := consumer.Poll(100)
		if ev == nil {
			continue
		}

		switch e := ev.(type) {
		case *kafka.Message:
			payload := e.Value
			pos := 0

			// Decode
			count := readVarint(payload, &pos)
			var sum int64
			for count != 0 {
				absCount := count
				if count < 0 {
					absCount = -count
					_ = readVarint(payload, &pos) // skip byte size
				}
				for j := 0; j < int(absCount); j++ {
					sum += readVarint(payload, &pos)
				}
				count = readVarint(payload, &pos)
			}

			// Encode
			outBuf = outBuf[:0]
			outBuf = writeVarint(outBuf, sum)

			producer.Produce(&kafka.Message{
				TopicPartition: kafka.TopicPartition{Topic: &outputTopic, Partition: kafka.PartitionAny},
				Value:          outBuf,
				Key:            e.Key,
			}, nil)

			localCounter++
			if localCounter >= 1000 {
				atomic.AddUint64(&messageCounter, localCounter)
				localCounter = 0
			}

		case kafka.Error:
			fmt.Fprintf(os.Stderr, "Worker %d error: %v\n", id, e)
		}
	}
}

func getEnv(key, fallback string) string {
	if value, ok := os.LookupEnv(key); ok {
		return value
	}
	return fallback
}
