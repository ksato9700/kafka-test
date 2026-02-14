.PHONY: run-consumer run-producer docker-build docker-run-consumer docker-run-producer

run-consumer:
	cargo run --bin kafka-consumer

run-producer:
	cargo run --bin kafka-producer

docker-build:
	docker build -t kafka-test-rust .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-rust kafka-consumer

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-rust kafka-producer
