package com.example.kafka

import org.apache.kafka.clients.producer.{KafkaProducer, ProducerConfig, ProducerRecord, Callback, RecordMetadata}
import org.apache.kafka.common.serialization.StringSerializer
import org.slf4j.LoggerFactory
import java.util.Properties
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.TimeUnit

object Producer {
  private val logger = LoggerFactory.getLogger(getClass)

  def main(args: Array[String]): Unit = {
    val bootstrapServers = sys.env.getOrElse("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
    val topic = sys.env.getOrElse("TOPIC_NAME", "my-topic-1")

    logger.info("Hello from kafka-test-scala!")
    logger.info(s"🛠️ Connecting KafkaProducer to topic '$topic' at '$bootstrapServers'...")

    val props = new Properties()
    props.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
    props.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, classOf[StringSerializer].getName)
    props.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, classOf[StringSerializer].getName)
    props.put(ProducerConfig.ACKS_CONFIG, "all")
    props.put(ProducerConfig.LINGER_MS_CONFIG, "0")

    // Force IPv4 for local dev
    System.setProperty("java.net.preferIPv4Stack", "true")

    val producer = new KafkaProducer[String, String](props)
    val running = new AtomicBoolean(true)

    Runtime.getRuntime.addShutdownHook(new Thread(() => {
      logger.info("🛑 Shutting down producer...")
      running.set(false)
      producer.close()
      logger.info("✅ Producer closed.")
    }))

    logger.info("🚀 Producer is now running...")

    var messageId = 0L
    try {
      while (running.get()) {
        val eventTime = System.currentTimeMillis() / 1000.0
        val content = f"""{"message_id":$messageId,"event_time":$eventTime%.3f,"content":"Message $messageId"}"""

        producer.send(new ProducerRecord[String, String](topic, content), new Callback {
          override def onCompletion(metadata: RecordMetadata, exception: Exception): Unit = {
            if (exception != null) {
              logger.error("❌ Error sending message: ", exception)
            } else {
              logger.info(s"🚀 Sent: $content")
            }
          }
        })

        messageId += 1
        TimeUnit.SECONDS.sleep(2)
      }
    } catch {
      case e: InterruptedException => Thread.currentThread().interrupt()
      case e: Exception => logger.error("❌ Unexpected error: ", e)
    } finally {
      producer.close()
    }
  }
}
