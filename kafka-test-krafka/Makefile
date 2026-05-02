.PHONY: run-consumer run-producer docker-build docker-run-consumer docker-run-producer

run-consumer:
	cargo run --bin kafka-consumer

run-producer:
	cargo run --bin kafka-producer

docker-build:
	cd .. && docker build -f kafka-test-krafka/Dockerfile -t kafka-test-krafka .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-krafka kafka-consumer

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-krafka kafka-producer
