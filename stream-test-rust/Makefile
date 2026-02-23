.PHONY: build run-stream docker-build docker-run

build:
	cargo build --release

run-stream:
	cargo run --release --bin processor

run-producer:
	cargo run --release --bin producer

run-consumer:
	cargo run --release --bin consumer

run-benchmark:
	BENCHMARK=1 NUM_WORKERS=8 cargo run --release --bin processor

docker-build:
	cd .. && docker build -f stream-test-rust/Dockerfile -t stream-test-rust .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-rust
