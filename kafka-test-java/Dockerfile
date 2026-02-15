FROM gradle:8.6-jdk21 AS builder

WORKDIR /app
COPY build.gradle settings.gradle ./
COPY src ./src
RUN gradle installDist --no-daemon

FROM eclipse-temurin:21-jre-alpine

WORKDIR /app
COPY --from=builder /app/build/install/kafka-test-java /app

# The installDist creates a start script in bin/
ENV PATH="/app/bin:${PATH}"

CMD ["kafka-test-java"]
