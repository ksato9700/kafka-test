FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y 
    build-essential 
    librdkafka-dev 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY stream-test-c/src ./src
COPY stream-test-c/Makefile ./
RUN make

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y 
    librdkafka1 
    ca-certificates 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/stream-test-c .

CMD ["./stream-test-c"]
