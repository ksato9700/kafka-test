FROM sbtscala/scala-sbt:eclipse-temurin-jammy-17.0.10_7_1.9.9_3.4.1 AS builder

WORKDIR /app
COPY . .
RUN sbt stage

FROM eclipse-temurin:21-jre-alpine

WORKDIR /app
COPY --from=builder /app/target/universal/stage /app

ENV PATH="/app/bin:${PATH}"

CMD ["kafka-test-scala"]
