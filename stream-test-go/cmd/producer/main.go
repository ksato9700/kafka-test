package main

import (
	"fmt"
	"math/rand"
	"os"
	"strconv"
	"sync/atomic"
	"time"

	"github.com/confluentinc/confluent-kafka-go/v2/kafka"
)

var sendCounter uint64

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
	topic := getEnv("INPUT_TOPIC", "integer-list-input-avro")
	numProducers, _ := strconv.Atoi(getEnv("PRODUCER_THREADS", "3"))

	go func() {
		ticker := time.NewTicker(5 * time.Second)
		for range ticker.C {
			count := atomic.SwapUint64(&sendCounter, 0)
			if count > 0 {
				fmt.Printf("📤 Go Sending Throughput: %d messages/sec\n", count/5)
			}
		}
	}()

	fmt.Printf("🚀 Starting High-Performance Go Producer with %d threads...\n", numProducers)

	for i := 0; i < numProducers; i++ {
		go func(id int) {
			producer, err := kafka.NewProducer(&kafka.ConfigMap{
				"bootstrap.servers":            bootstrapServers,
				"broker.address.family":        "v4",
				"compression.type":             "snappy",
				"linger.ms":                    20,
				"batch.num.messages":           100000,
				"queue.buffering.max.messages": 1000000,
			})
			if err != nil {
				panic(err)
			}
			defer producer.Close()

			fmt.Printf("Producer thread %d started\n", id)
			r := rand.New(rand.NewSource(time.Now().UnixNano() + int64(id)))
			buf := make([]byte, 0, 128)

			for {
				buf = buf[:0]
				count := int64(r.Intn(4) + 2) // 2-5

				buf = writeVarint(buf, count)
				for j := 0; j < int(count); j++ {
					buf = writeVarint(buf, int64(r.Intn(100)))
				}
				buf = writeVarint(buf, 0)

				producer.Produce(&kafka.Message{
					TopicPartition: kafka.TopicPartition{Topic: &topic, Partition: kafka.PartitionAny},
					Value:          buf,
				}, nil)

				atomic.AddUint64(&sendCounter, 1)
			}
		}(i)
	}

	select {}
}

func getEnv(key, fallback string) string {
	if value, ok := os.LookupEnv(key); ok {
		return value
	}
	return fallback
}
