package com.example.kafka

import org.apache.avro.Schema
import org.apache.avro.generic.{GenericData, GenericDatumWriter, GenericRecord}
import org.apache.avro.io.EncoderFactory
import org.apache.kafka.clients.producer.{Callback, KafkaProducer, ProducerConfig, ProducerRecord, RecordMetadata}
import org.apache.kafka.common.serialization.{ByteArraySerializer, StringSerializer}
import org.slf4j.LoggerFactory

import java.io.{ByteArrayOutputStream, File}
import java.nio.file.{Files, Paths}
import java.util.Properties
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.TimeUnit

object Producer {
  private val logger = LoggerFactory.getLogger(getClass)

  def loadSchema(): Schema = {
    val paths = Seq(
      "../schemas/message.avsc",
      "/app/schemas/message.avsc",
      "schemas/message.avsc"
    )

    paths.find(p => Files.exists(Paths.get(p))) match {
      case Some(p) => new Schema.Parser().parse(new File(p))
      case None => throw new RuntimeException(s"Schema file not found in paths: $paths")
    }
  }

  def main(args: Array[String]): Unit = {
    try {
      val schema = loadSchema()
      logger.info(s"Loaded Avro schema: ${schema.getFullName}")

      val bootstrapServers = sys.env.getOrElse("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
      val topic = sys.env.getOrElse("TOPIC_NAME", "my-topic-1")

      logger.info("Hello from kafka-test-scala!")
      logger.info(s"🛠️ Connecting KafkaProducer to topic '$topic' at '$bootstrapServers'...")

      val props = new Properties()
      props.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
      props.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, classOf[StringSerializer].getName)
      props.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, classOf[ByteArraySerializer].getName)
      props.put(ProducerConfig.ACKS_CONFIG, "all")
      props.put(ProducerConfig.LINGER_MS_CONFIG, "0")

      // Force IPv4 for local dev
      System.setProperty("java.net.preferIPv4Stack", "true")

      val producer = new KafkaProducer[String, Array[Byte]](props)
      val running = new AtomicBoolean(true)

      Runtime.getRuntime.addShutdownHook(new Thread(() => {
        logger.info("🛑 Shutting down producer...")
        running.set(false)
        producer.close()
        logger.info("✅ Producer closed.")
      }))

      logger.info("🚀 Producer is now running...")

      var messageId = 0L
      val writer = new GenericDatumWriter[GenericRecord](schema)

      while (running.get()) {
        val eventTime = System.currentTimeMillis() / 1000.0
        val content = s"Message $messageId"

        val avroRecord = new GenericData.Record(schema)
        avroRecord.put("message_id", messageId)
        avroRecord.put("event_time", eventTime)
        avroRecord.put("content", content)

        val out = new ByteArrayOutputStream()
        val encoder = EncoderFactory.get().binaryEncoder(out, null)
        writer.write(avroRecord, encoder)
        encoder.flush()
        val payload = out.toByteArray

        producer.send(new ProducerRecord[String, Array[Byte]](topic, payload), new Callback {
          override def onCompletion(metadata: RecordMetadata, exception: Exception): Unit = {
            if (exception != null) {
              logger.error("❌ Error sending message: ", exception)
            } else {
              logger.info(s"🚀 Sent: $avroRecord")
            }
          }
        })

        messageId += 1
        TimeUnit.SECONDS.sleep(2)
      }
    } catch {
      case e: InterruptedException => Thread.currentThread().interrupt()
      case e: Exception => logger.error("❌ Unexpected error: ", e)
    }
  }
}
