.PHONY: run-stream run-producer run-consumer docker-build docker-run

run-stream:
	bundle exec ruby --yjit bin/processor.rb

run-producer:
	bundle exec ruby --yjit bin/producer.rb

run-consumer:
	bundle exec ruby --yjit bin/consumer.rb

docker-build:
	cd .. && docker build -f stream-test-ruby/Dockerfile -t stream-test-ruby .

docker-run: docker-build
	docker run --rm --network kafka-net -e KAFKA_BOOTSTRAP_SERVERS="kafka-broker:9092" stream-test-ruby
