.PHONY: run-consumer run-producer docker-build docker-run-consumer docker-run-producer

run-consumer:
	sbt "runMain com.example.kafka.Consumer"

run-producer:
	sbt "runMain com.example.kafka.Producer"

docker-build:
	docker build -t kafka-test-scala .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-scala java -cp "lib/*" com.example.kafka.Consumer

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-scala java -cp "lib/*" com.example.kafka.Producer
