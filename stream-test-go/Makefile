.PHONY: run-stream run-producer run-consumer docker-build docker-run

NUM_WORKERS ?= 8
PRODUCER_THREADS ?= 2

run-stream:
	NUM_WORKERS=$(NUM_WORKERS) go run cmd/processor/main.go

run-producer:
	PRODUCER_THREADS=$(PRODUCER_THREADS) go run cmd/producer/main.go

run-consumer:
	go run cmd/consumer/main.go

run-benchmark:
	BENCHMARK=1 NUM_WORKERS=8 go run cmd/processor/main.go

docker-build:
	cd .. && docker build -f stream-test-go/Dockerfile -t stream-test-go .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-go
