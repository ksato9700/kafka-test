.PHONY: run-consumer run-producer docker-build docker-run-consumer docker-run-producer

run-consumer:
	uv run python consumer.py

run-producer:
	uv run python producer.py

docker-build:
	cd .. && docker build -f kafka-test-python/Dockerfile -t kafka-test-python .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-python python consumer.py

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-python python producer.py
