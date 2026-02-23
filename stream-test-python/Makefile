.PHONY: run-stream run-producer run-consumer docker-build docker-run

run-stream:
	uv run bin/processor.py

run-producer:
	uv run bin/producer.py

run-consumer:
	uv run bin/consumer.py

run-benchmark:
	BENCHMARK=1 NUM_WORKERS=8 uv run bin/processor.py

docker-build:
	cd .. && docker build -f stream-test-python/Dockerfile -t stream-test-python .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-python
