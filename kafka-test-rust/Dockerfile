FROM rust:1-alpine3.23 AS builder

WORKDIR /usr/src/app

# Install build dependencies
# zlib-dev provides libz.a for static linking
# openssl-dev provides libssl.a/libcrypto.a for static linking
# cmake/make/g++ required for building rdkafka from source
RUN apk add --no-cache musl-dev openssl-dev pkgconfig cmake make g++ zlib-dev cyrus-sasl-dev curl-dev zlib-static openssl-libs-static

COPY . .

# Build statically linked binary
RUN cargo build --release

FROM alpine:3.23

WORKDIR /app

# Install runtime dependencies
# Since we link statically, we don't need librdkafka.so
# But openssl might need dynamic libs if not fully static (rust-openssl usually links against .a if found)
# cyrus-sasl might be needed if SASL is used dynamically
RUN apk add --no-cache libgcc libstdc++

COPY --from=builder /usr/src/app/target/release/kafka-consumer /usr/local/bin/kafka-consumer
COPY --from=builder /usr/src/app/target/release/kafka-producer /usr/local/bin/kafka-producer

CMD ["kafka-consumer"]
