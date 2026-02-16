package main

import (
	"fmt"
	"os"
	"os/signal"
	"strconv"
	"sync"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/confluentinc/confluent-kafka-go/v2/kafka"
)

var messageCounter uint64

// Buffer pool to reduce GC pressure
var bufPool = sync.Pool{
	New: func() interface{} {
		return make([]byte, 0, 32)
	},
}

func main() {
	bootstrapServers := getEnv("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
	inputTopic := getEnv("INPUT_TOPIC", "integer-list-input-avro")
	outputTopic := getEnv("OUTPUT_TOPIC", "integer-sum-output-avro")
	groupID := getEnv("APPLICATION_ID", "stream-sum-test-go-avro")
	numWorkers, _ := strconv.Atoi(getEnv("NUM_WORKERS", "8"))

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

	fmt.Printf("🚀 Starting Ultra-Performance Go Stream Processor with %d workers...\n", numWorkers)

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
		"queued.min.messages":      1000000,
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

	// Handle delivery reports to return buffers to the pool
	go func() {
		for e := range producer.Events() {
			switch ev := e.(type) {
			case *kafka.Message:
				if ev.Value != nil {
					// In a real app, we would return to pool here, 
					// but confluent-kafka-go takes ownership.
					// We'll just let GC handle it or use a different strategy.
				}
			}
		}
	}()

	consumer.Subscribe(inputTopic, nil)

	fmt.Printf("Worker %d started\n", id)
	var localCounter uint64

	for {
		ev := consumer.Poll(10)
		if ev == nil {
			continue
		}

		switch e := ev.(type) {
		case *kafka.Message:
			payload := e.Value
			pos := 0
			
			// Inline Zig-Zag Decoding
			var val uint64
			var shift uint
			for {
				b := payload[pos]
				pos++
				val |= uint64(b&0x7F) << shift
				if b&0x80 == 0 { break }
				shift += 7
			}
			count := int64(val>>1) ^ -int64(val&1)

			var sum int64
			for count != 0 {
				absCount := count
				if count < 0 {
					absCount = -count
					// skip byte size
					var v_s uint64; var s_s uint
					for {
						b := payload[pos]; pos++
						v_s |= uint64(b&0x7F) << s_s
						if b&0x80 == 0 { break }; s_s += 7
					}
				}
				for j := 0; j < int(absCount); j++ {
					var v_e uint64; var s_e uint
					for {
						b := payload[pos]; pos++
						v_e |= uint64(b&0x7F) << s_e
						if b&0x80 == 0 { break }; s_e += 7
					}
					sum += int64(v_e>>1) ^ -int64(v_e&1)
				}
				// next block
				var v_n uint64; var s_n uint
				for {
					b := payload[pos]; pos++
					v_n |= uint64(b&0x7F) << s_n
					if b&0x80 == 0 { break }; s_n += 7
				}
				count = int64(v_n>>1) ^ -int64(v_n&1)
			}

			// Inline Zig-Zag Encoding
			v_enc := uint64((sum << 1) ^ (sum >> 63))
			
			// Since confluent-kafka-go copies the buffer if we don't use 
			// the 'unsafe' approach, we try to minimize the size of the allocation.
			var outBuf []byte
			if v_enc < 0x80 {
				outBuf = []byte{uint8(v_enc)}
			} else {
				outBuf = make([]byte, 0, 8)
				for v_enc >= 0x80 {
					outBuf = append(outBuf, uint8(v_enc)|0x80)
					v_enc >>= 7
				}
				outBuf = append(outBuf, uint8(v_enc))
			}

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
			if e.Code() != kafka.ErrPartitionEOF {
				fmt.Fprintf(os.Stderr, "Worker %d error: %v\n", id, e)
			}
		}
	}
}

func getEnv(key, fallback string) string {
	if value, ok := os.LookupEnv(key); ok {
		return value
	}
	return fallback
}
