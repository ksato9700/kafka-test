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

func main() {
	bootstrapServers := getEnv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
	topic := getEnv("INPUT_TOPIC", "integer-list-input-avro")
	numProducers, _ := strconv.Atoi(getEnv("PRODUCER_THREADS", "4"))

	go func() {
		ticker := time.NewTicker(5 * time.Second)
		for range ticker.C {
			count := atomic.SwapUint64(&sendCounter, 0)
			if count > 0 {
				fmt.Printf("📤 Go Sending Throughput: %d messages/sec\n", count/5)
			}
		}
	}()

	fmt.Printf("🚀 Starting Ultra-Performance Go Producer with %d threads...\n", numProducers)

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

			r := rand.New(rand.NewSource(time.Now().UnixNano() + int64(id)))

			for {
				count := int64(r.Intn(4) + 2) // 2-5
				// Allocate new slice for every message to avoid race conditions in Produce()
				buf := make([]byte, 0, 32)

				// Encode count
				v_c := uint64((count << 1) ^ (count >> 63))
				for v_c >= 0x80 {
					buf = append(buf, uint8(v_c)|0x80)
					v_c >>= 7
				}
				buf = append(buf, uint8(v_c))

				for j := 0; j < int(count); j++ {
					n := int64(r.Intn(100))
					v_n := uint64((n << 1) ^ (n >> 63))
					for v_n >= 0x80 {
						buf = append(buf, uint8(v_n)|0x80)
						v_n >>= 7
					}
					buf = append(buf, uint8(v_n))
				}
				buf = append(buf, 0) // End of array

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
