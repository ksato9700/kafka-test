.PHONY: run-stream run-producer run-consumer docker-build docker-run

STREAMS_NUM_STREAM_THREADS ?= 8
PRODUCER_THREADS ?= 2

run-stream:
	STREAMS_NUM_STREAM_THREADS=$(STREAMS_NUM_STREAM_THREADS) sbt "runMain com.example.kafka.StreamSumApp"

run-producer:
	PRODUCER_THREADS=$(PRODUCER_THREADS) sbt "runMain com.example.kafka.TestProducer"

run-consumer:
	sbt "runMain com.example.kafka.TestConsumer"

docker-build:
	cd .. && docker build -f stream-test-scala/Dockerfile -t stream-test-scala .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-scala
