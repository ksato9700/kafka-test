FROM golang:1.22-bookworm AS builder

RUN apt-get update && apt-get install -y 
    build-essential 
    pkg-config 
    librdkafka-dev 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY stream-test-go/go.mod stream-test-go/go.sum ./
RUN go mod download

COPY stream-test-go/cmd ./cmd
COPY schemas /app/schemas
RUN go build -o stream-test-go cmd/processor/main.go

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y 
    librdkafka1 
    ca-certificates 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/stream-test-go /app/stream-test-go
COPY --from=builder /app/schemas /app/schemas

CMD ["./stream-test-go"]
