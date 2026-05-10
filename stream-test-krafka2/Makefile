.PHONY: build run-benchmark run-stream docker-build docker-run

BOOTSTRAP ?= 127.0.0.1:9094

build:
	cargo build --release

run-benchmark:
	BENCHMARK=1 NUM_WORKERS=8 cargo run --release --bin processor

run-benchmark-skip-load:
	BENCHMARK=1 SKIP_LOAD=1 NUM_WORKERS=8 cargo run --release --bin processor

run-stream:
	cargo run --release --bin processor

docker-build:
	cd .. && docker build -f stream-test-krafka2/Dockerfile -t stream-test-krafka2 .

docker-run:
	docker run --rm --network kafka-net \
	  -e BOOTSTRAP=kafka-broker:9092 \
	  -e BENCHMARK=1 \
	  stream-test-krafka2
