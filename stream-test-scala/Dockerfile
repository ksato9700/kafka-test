FROM sbt-builder:latest AS builder

WORKDIR /app
COPY stream-test-scala/build.sbt ./
COPY stream-test-scala/project ./project
RUN sbt update

COPY stream-test-scala/src ./src
RUN sbt assembly

FROM eclipse-temurin:21-jre-alpine

WORKDIR /app
COPY --from=builder /app/target/scala-3.3.1/stream-test-scala-assembly-0.1.0.jar ./app.jar

CMD ["java", "-jar", "app.jar"]
