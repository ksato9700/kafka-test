.PHONY: build run-benchmark run-benchmark-durable run-stream run-producer run-consumer docker-build docker-run

BOOTSTRAP ?= 127.0.0.1:9094

build:
	cargo build --release

run-benchmark:
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-list-input-benchmark --partitions 8 --replication-factor 1
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-sum-output-benchmark --partitions 8 --replication-factor 1
	BENCHMARK=1 NUM_WORKERS=8 cargo run --release --bin processor

run-benchmark-durable:
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-list-input-benchmark --partitions 8 --replication-factor 1
	docker exec kafka-broker /opt/kafka/bin/kafka-topics.sh \
	  --bootstrap-server localhost:9092 --create --if-not-exists \
	  --topic integer-sum-output-benchmark --partitions 8 --replication-factor 1
	BENCHMARK=1 DURABLE=1 NUM_WORKERS=8 cargo run --release --bin processor

run-stream:
	cargo run --release --bin processor

run-producer:
	cargo run --release --bin producer

run-consumer:
	cargo run --release --bin consumer

docker-build:
	cd .. && docker build -f stream-test-krafka/Dockerfile -t stream-test-krafka .

docker-run:
	docker run --rm --network kafka-net \
	  -e BOOTSTRAP=kafka-broker:9092 \
	  stream-test-krafka
