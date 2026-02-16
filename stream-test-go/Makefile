.PHONY: run-stream run-producer run-consumer docker-build docker-run

run-stream:
	go run cmd/processor/main.go

run-producer:
	go run cmd/producer/main.go

run-consumer:
	go run cmd/consumer/main.go

docker-build:
	cd .. && docker build -f stream-test-go/Dockerfile -t stream-test-go .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-go
