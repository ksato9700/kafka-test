.PHONY: run-consumer run-producer docker-build docker-run-consumer docker-run-producer

run-consumer:
	gradle runConsumer --console=plain

run-producer:
	gradle runProducer --console=plain

docker-build:
	cd .. && docker build -f kafka-test-java/Dockerfile -t kafka-test-java .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-java java -cp "lib/*" com.example.kafka.Consumer

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-java java -cp "lib/*" com.example.kafka.Producer
