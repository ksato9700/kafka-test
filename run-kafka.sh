#!/bin/sh
docker run -d --rm \
	--name kafka-broker \
	--publish 9092:9092 \
	apache/kafka:latest
