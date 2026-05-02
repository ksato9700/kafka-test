FROM rust:1-alpine3.23 AS builder

WORKDIR /usr/src/app

RUN apk add --no-cache musl-dev

COPY kafka-test-krafka .

RUN cargo build --release

FROM alpine:3.23

WORKDIR /app

COPY schemas /app/schemas
COPY --from=builder /usr/src/app/target/release/kafka-consumer /usr/local/bin/kafka-consumer
COPY --from=builder /usr/src/app/target/release/kafka-producer /usr/local/bin/kafka-producer

CMD ["kafka-consumer"]
