name := "stream-test-scala"
version := "0.1.0"
scalaVersion := "3.3.1"

libraryDependencies ++= Seq(
  "org.apache.kafka" % "kafka-clients" % "3.7.0",
  "org.slf4j" % "slf4j-api" % "2.0.12",
  "ch.qos.logback" % "logback-classic" % "1.5.3"
)

enablePlugins(JavaAppPackaging)

Compile / mainClass := Some("com.example.kafka.StreamSumApp")
