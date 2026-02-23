package main

import (
	"fmt"
	"math/rand"
	"os"
	"os/signal"
	"strconv"
	"sync/atomic"
	"syscall"
	"time"

	"github.com/confluentinc/confluent-kafka-go/v2/kafka"
)

var processedCounter uint64
const totalRecords = 50000000

func readVarint(data []byte, pos *int) int64 {
	var val uint64
	var shift uint
	for {
		b := data[*pos]
		*pos++
		val |= uint64(b&0x7F) << shift
		if b&0x80 == 0 { break }
		shift += 7
	}
	return int64(val>>1) ^ -int64(val&1)
}

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
	inputTopic := "integer-list-input-benchmark"
	outputTopic := "integer-sum-output-benchmark"
	numWorkers, _ := strconv.Atoi(getEnv("NUM_WORKERS", "8"))

	if os.Getenv("BENCHMARK") != "" {
		fmt.Printf("🚀 Phase 1: Loading %d records (Go)...\n", totalRecords)
		p, _ := kafka.NewProducer(&kafka.ConfigMap{
			"bootstrap.servers":            bootstrapServers,
			"broker.address.family":        "v4",
			"compression.type":             "snappy",
			"linger.ms":                    20,
			"batch.num.messages":           100000,
			"queue.buffering.max.messages": 1000000,
		})
		
		// Drain events to prevent deadlock
		go func() {
			for range p.Events() {
			}
		}()

		r := rand.New(rand.NewSource(time.Now().UnixNano()))
		for i := 0; i < totalRecords; i++ {
			buf := make([]byte, 0, 32)
			count := int64(r.Intn(4) + 2)
			buf = writeVarint(buf, count)
			for j := 0; j < int(count); j++ { buf = writeVarint(buf, int64(r.Intn(100))) }
			buf = writeVarint(buf, 0)
			
			for {
				err := p.Produce(&kafka.Message{TopicPartition: kafka.TopicPartition{Topic: &inputTopic, Partition: kafka.PartitionAny}, Value: buf}, nil)
				if err != nil {
					if err.(kafka.Error).Code() == kafka.ErrQueueFull {
						time.Sleep(10 * time.Millisecond)
						continue
					}
					fmt.Printf("Produce error: %v\n", err)
				}
				break
			}
		}
		fmt.Println("Flushing producer...")
		p.Flush(60000)
		p.Close()
		fmt.Println("✅ Phase 1 Complete.")
	}

	fmt.Printf("🚀 Phase 2: Processing backlog with %d workers...\n", numWorkers)
	groupID := fmt.Sprintf("benchmark-go-%d", time.Now().UnixNano())

	sigchan := make(chan os.Signal, 1)
	signal.Notify(sigchan, syscall.SIGINT, syscall.SIGTERM)

	for i := 0; i < numWorkers; i++ {
		go startWorker(i, bootstrapServers, inputTopic, outputTopic, groupID)
	}

	start := time.Now()
	// Progress reporter
	go func() {
		for {
			time.Sleep(1 * time.Second)
			c := atomic.LoadUint64(&processedCounter)
			if c > 0 { fmt.Printf("Progress: %d%% (%d / %d)\n", c*100/totalRecords, c, totalRecords) }
			if c >= totalRecords { break }
		}
	}()

	for atomic.LoadUint64(&processedCounter) < totalRecords {
		time.Sleep(100 * time.Millisecond)
	}

	duration := time.Since(start)
	fmt.Printf("🏁 BENCHMARK RESULT (GO) 🏁\n")
	fmt.Printf("Total Records: %d\n", totalRecords)
	fmt.Printf("Processing Time: %.2fs\n", duration.Seconds())
	fmt.Printf("Throughput:      %.0f msg/sec\n", float64(totalRecords)/duration.Seconds())
	os.Exit(0)
}

func startWorker(id int, bootstrap, inputTopic, outputTopic, groupID string) {
	config := &kafka.ConfigMap{
		"bootstrap.servers":     bootstrap,
		"broker.address.family": "v4",
		"group.id":              groupID,
		"auto.offset.reset":     "earliest",
		"fetch.min.bytes":       1000000,
		"queued.min.messages":   2000000,
	}
	c, _ := kafka.NewConsumer(config)
	defer c.Close()
	p, _ := kafka.NewProducer(&kafka.ConfigMap{
		"bootstrap.servers":            bootstrap,
		"broker.address.family":        "v4",
		"compression.type":             "snappy",
		"linger.ms":                    20,
		"batch.num.messages":           100000,
		"queue.buffering.max.messages": 1000000,
	})
	defer p.Close()

	// Drain events to prevent deadlock
	go func() {
		for range p.Events() {
		}
	}()

	c.Subscribe(inputTopic, nil)
	var localCounter uint64

	for {
		if atomic.LoadUint64(&processedCounter) >= totalRecords {
			p.Flush(5000)
			return
		}
		ev := c.Poll(100)
		if ev == nil { continue }

		switch e := ev.(type) {
		case *kafka.Message:
			payload := e.Value
			pos := 0
			var v uint64; var s uint
			for {
				b := payload[pos]; pos++
				v |= uint64(b&0x7F) << s
				if b&0x80 == 0 { break }; s += 7
			}
			count := int64(v>>1) ^ -int64(v&1)
			var sum int64
			for count != 0 {
				absCount := count
				if count < 0 { absCount = -count; var v_s uint64; var s_s uint
					for { b := payload[pos]; pos++; v_s |= uint64(b&0x7F) << s_s; if b&0x80 == 0 { break }; s_s += 7 }
				}
				for j := 0; j < int(absCount); j++ {
					var v_e uint64; var s_e uint
					for { b := payload[pos]; pos++; v_e |= uint64(b&0x7F) << s_e; if b&0x80 == 0 { break }; s_e += 7 }
					sum += int64(v_e>>1) ^ -int64(v_e&1)
				}
				var v_n uint64; var s_n uint
				for { b := payload[pos]; pos++; v_n |= uint64(b&0x7F) << s_n; if b&0x80 == 0 { break }; s_n += 7 }
				count = int64(v_n>>1) ^ -int64(v_n&1)
			}

			out := make([]byte, 0, 8)
			v_enc := uint64((sum << 1) ^ (sum >> 63))
			for v_enc >= 0x80 { out = append(out, uint8(v_enc)|0x80); v_enc >>= 7 }
			out = append(out, uint8(v_enc))

			for {
				err := p.Produce(&kafka.Message{TopicPartition: kafka.TopicPartition{Topic: &outputTopic, Partition: kafka.PartitionAny}, Value: out, Key: e.Key}, nil)
				if err != nil {
					if err.(kafka.Error).Code() == kafka.ErrQueueFull {
						time.Sleep(10 * time.Millisecond)
						continue
					}
				}
				break
			}

			localCounter++
			if localCounter >= 1000 {
				atomic.AddUint64(&processedCounter, localCounter)
				localCounter = 0
			}
		}
	}
}

func getEnv(key, fallback string) string {
	if v, ok := os.LookupEnv(key); ok { return v }
	return fallback
}
