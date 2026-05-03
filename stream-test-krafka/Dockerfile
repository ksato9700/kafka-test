FROM rust:1-alpine3.23 AS builder
WORKDIR /usr/src/app
RUN apk add --no-cache musl-dev
COPY stream-test-krafka .
RUN cargo build --release

FROM alpine:3.23
WORKDIR /app
COPY --from=builder /usr/src/app/target/release/processor /usr/local/bin/processor
COPY --from=builder /usr/src/app/target/release/producer /usr/local/bin/producer
COPY --from=builder /usr/src/app/target/release/consumer /usr/local/bin/consumer
CMD ["processor"]
