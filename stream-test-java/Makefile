.PHONY: run-stream run-producer run-consumer docker-build docker-run

STREAMS_NUM_STREAM_THREADS ?= 8

run-stream:
	STREAMS_NUM_STREAM_THREADS=$(STREAMS_NUM_STREAM_THREADS) gradle runStream --console=plain

run-producer:
	gradle runProducer --console=plain

run-consumer:
	gradle runConsumer --console=plain

run-benchmark:
	STREAMS_NUM_STREAM_THREADS=$(STREAMS_NUM_STREAM_THREADS) gradle runBenchmark --console=plain

run-monitor:
	gradle runMonitor --console=plain -Pargs="$(GROUP)"

docker-build:
	cd .. && docker build -f stream-test-java/Dockerfile -t stream-test-java .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-java
