package com.example.kafka

import org.apache.kafka.clients.producer.{KafkaProducer, ProducerConfig, ProducerRecord}
import org.apache.kafka.common.serialization.ByteArraySerializer
import org.slf4j.{Logger, LoggerFactory}

import java.io.ByteArrayOutputStream
import java.util.Properties
import java.util.concurrent.{Executors, ThreadLocalRandom, TimeUnit}
import java.util.concurrent.atomic.AtomicLong

object TestProducer:
  private val logger: Logger = LoggerFactory.getLogger(getClass)
  private val sendCounter = new AtomicLong(0)

  private def writeVarint(out: ByteArrayOutputStream, n: Long): Unit =
    var val_res = (n << 1) ^ (n >> 63)
    while (val_res >= 0x80) {
      out.write((val_res | 0x80).toInt)
      val_res >>>= 7
    }
    out.write(val_res.toInt)

  def main(args: Array[String]): Unit =
    val bootstrapServers = sys.env.getOrElse("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
    val topic = sys.env.getOrElse("INPUT_TOPIC", "integer-list-input-avro")
    val numThreads = sys.env.getOrElse("PRODUCER_THREADS", "2").toInt

    val reporter = Executors.newSingleThreadScheduledExecutor()
    reporter.scheduleAtFixedRate(() => {
      val count = sendCounter.getAndSet(0)
      if (count > 0) logger.info(s"📤 Scala Sending Throughput: ${count / 5} messages/sec")
    }, 5, 5, TimeUnit.SECONDS)

    logger.info(s"🚀 Starting Ultra-Performance Scala Producers: $numThreads")

    for i <- 0 until numThreads do
      val threadId = i
      new Thread(() => {
        val props = new Properties()
        props.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
        props.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, classOf[ByteArraySerializer].getName)
        props.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, classOf[ByteArraySerializer].getName)
        props.put(ProducerConfig.ACKS_CONFIG, "1")
        props.put(ProducerConfig.BATCH_SIZE_CONFIG, "131072")
        props.put(ProducerConfig.LINGER_MS_CONFIG, "20")
        props.put(ProducerConfig.COMPRESSION_TYPE_CONFIG, "snappy")
        props.put(ProducerConfig.BUFFER_MEMORY_CONFIG, "67108864")

        val producer = new KafkaProducer[Array[Byte], Array[Byte]](props)
        val out = new ByteArrayOutputStream(128)
        val random = ThreadLocalRandom.current()

        logger.info(s"Producer thread $threadId started")

        try
          while true do
            out.reset()
            val count = random.nextInt(2, 6)
            writeVarint(out, count.toLong)
            var j = 0
            while j < count do
              writeVarint(out, random.nextInt(0, 100).toLong)
              j += 1
            writeVarint(out, 0L)

            producer.send(new ProducerRecord(topic, null, out.toByteArray))
            sendCounter.incrementAndGet()
        catch
          case e: Exception => logger.error(s"Producer thread $threadId failed", e)
        finally
          producer.close()
      }).start()
