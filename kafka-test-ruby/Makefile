.PHONY: install run-consumer run-producer docker-build docker-run-consumer docker-run-producer

install:
	bundle install

run-consumer:
	bundle exec ruby consumer.rb

run-producer:
	bundle exec ruby producer.rb

docker-build:
	cd .. && docker build -f kafka-test-ruby/Dockerfile -t kafka-test-ruby .

docker-run-consumer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-ruby ruby consumer.rb

docker-run-producer: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" kafka-test-ruby ruby producer.rb
