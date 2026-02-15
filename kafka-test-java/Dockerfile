FROM gradle:8.6-jdk21 AS builder

WORKDIR /app
COPY kafka-test-java/build.gradle kafka-test-java/settings.gradle ./
COPY kafka-test-java/src ./src
COPY schemas /app/schemas
RUN gradle installDist --no-daemon

FROM eclipse-temurin:21-jre-alpine

WORKDIR /app
COPY --from=builder /app/build/install/kafka-test-java /app
COPY --from=builder /app/schemas /app/schemas

# The installDist creates a start script in bin/
ENV PATH="/app/bin:${PATH}"

CMD ["kafka-test-java"]
