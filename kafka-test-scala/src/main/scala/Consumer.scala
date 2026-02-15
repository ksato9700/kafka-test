package com.example.kafka

import org.apache.avro.Schema
import org.apache.avro.generic.{GenericDatumReader, GenericRecord}
import org.apache.avro.io.DecoderFactory
import org.apache.kafka.clients.consumer.{ConsumerConfig, KafkaConsumer}
import org.apache.kafka.common.errors.WakeupException
import org.apache.kafka.common.serialization.{ByteArrayDeserializer, StringDeserializer}
import org.slf4j.LoggerFactory

import java.io.File
import java.nio.file.{Files, Paths}
import java.time.Duration
import java.util.{Collections, Properties}
import java.util.concurrent.atomic.AtomicBoolean
import scala.jdk.CollectionConverters._

object Consumer {
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
      val groupId = sys.env.getOrElse("CONSUMER_GROUP_ID", "my-group-1")
      val offsetReset = sys.env.getOrElse("AUTO_OFFSET_RESET", "latest")

      logger.info(s"🛠️ Connecting KafkaConsumer to topic '$topic' at '$bootstrapServers' (group: '$groupId')...")

      val props = new Properties()
      props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
      props.put(ConsumerConfig.GROUP_ID_CONFIG, groupId)
      props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, classOf[StringDeserializer].getName)
      props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, classOf[ByteArrayDeserializer].getName)
      props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, offsetReset)
      props.put(ConsumerConfig.ENABLE_AUTO_COMMIT_CONFIG, "true")
      props.put(ConsumerConfig.SESSION_TIMEOUT_MS_CONFIG, "6000") // Fail fast

      // Force IPv4 for local dev
      System.setProperty("java.net.preferIPv4Stack", "true")

      val consumer = new KafkaConsumer[String, Array[Byte]](props)
      val closed = new AtomicBoolean(false)

      Runtime.getRuntime.addShutdownHook(new Thread(() => {
        logger.info("🛑 Shutting down gracefully...")
        closed.set(true)
        consumer.wakeup()
      }))

      val reader = new GenericDatumReader[GenericRecord](schema)

      try {
        consumer.subscribe(Collections.singletonList(topic))
        logger.info("✅ KafkaConsumer connected. Waiting for new messages...")
        logger.info("📥 Listening for messages...")

        while (!closed.get()) {
          val records = consumer.poll(Duration.ofMillis(100))
          for (record <- records.asScala) {
            try {
              val decoder = DecoderFactory.get().binaryDecoder(record.value(), null)
              val avroRecord = reader.read(null, decoder)

              val messageId = avroRecord.get("message_id").asInstanceOf[Long]
              val eventTime = avroRecord.get("event_time").asInstanceOf[Double]
              val content = avroRecord.get("content").toString

              val nowMillis = System.currentTimeMillis()
              val nowSeconds = nowMillis / 1000.0
              val latency = nowSeconds - eventTime

              logger.info(f"📨 New message: [ID=$messageId] $content at $eventTime%.3f")
              logger.info(f"⏱️ Latency: $latency%.3f seconds")
            } catch {
              case e: Exception => logger.warn(s"⚠️ Error parsing message: ${e.getMessage} - offset: ${record.offset()}")
            }
          }
        }
      } catch {
        case _: WakeupException => if (!closed.get()) throw new RuntimeException("Unexpected wakeup")
        case e: Exception => logger.error("❌ KafkaConsumer error: ", e)
      } finally {
        consumer.close()
        logger.info("✅ KafkaConsumer closed.")
      }
    } catch {
      case e: Exception => logger.error("❌ Fatal error: ", e)
    }
  }
}
