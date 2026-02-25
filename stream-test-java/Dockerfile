FROM gradle:8.6-jdk21 AS builder

WORKDIR /app
COPY stream-test-java/build.gradle stream-test-java/settings.gradle ./
COPY stream-test-java/src ./src
COPY schemas /app/schemas
RUN gradle installDist --no-daemon

FROM amazoncorretto:21-al2023

WORKDIR /app
COPY --from=builder /app/build/install/stream-test-java /app
COPY --from=builder /app/schemas /app/schemas

ENV PATH="/app/bin:${PATH}"

CMD ["stream-test-java"]
