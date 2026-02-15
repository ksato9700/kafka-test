name := "kafka-test-scala"
version := "0.1"
scalaVersion := "3.3.1"

libraryDependencies ++= Seq(
  "org.apache.kafka" % "kafka-clients" % "3.7.0",
  "org.apache.avro" % "avro" % "1.11.3",
  "org.slf4j" % "slf4j-api" % "2.0.12",
  "ch.qos.logback" % "logback-classic" % "1.5.3",
  "com.fasterxml.jackson.module" %% "jackson-module-scala" % "2.17.0"
)

enablePlugins(JavaAppPackaging) // For easy Docker packaging if available, but I'll do manual
