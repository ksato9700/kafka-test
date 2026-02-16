package com.example.kafka

import org.apache.kafka.clients.consumer.{ConsumerConfig, KafkaConsumer}
import org.apache.kafka.common.serialization.ByteArrayDeserializer
import org.slf4j.{Logger, LoggerFactory}

import java.io.ByteArrayInputStream
import java.time.Duration
import java.util.{Collections, Properties}
import java.util.concurrent.atomic.{AtomicBoolean, AtomicLong}
import java.util.concurrent.{Executors, TimeUnit}

object TestConsumer:
  private val logger: Logger = LoggerFactory.getLogger(getClass)
  private val resultCounter = new AtomicLong(0)
  private val running = new AtomicBoolean(true)

  private def readVarint(in: ByteArrayInputStream): Long =
    var val_res = 0L
    var shift = 0
    var b = in.read()
    while (b != -1) {
      val_res |= (b & 0x7F).toLong << shift
      if ((b & 0x80) == 0) return (val_res >>> 1) ^ -(val_res & 1)
      shift += 7
      b = in.read()
    }
    0L

  def main(args: Array[String]): Unit =
    val bootstrapServers = sys.env.getOrElse("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
    val topic = sys.env.getOrElse("OUTPUT_TOPIC", "integer-sum-output-avro")
    val numThreads = sys.env.getOrElse("CONSUMER_THREADS", "1").toInt

    val reporter = Executors.newSingleThreadScheduledExecutor()
    reporter.scheduleAtFixedRate(() => {
      val count = resultCounter.getAndSet(0)
      if (count > 0) logger.info(s"📥 Scala Result Throughput: ${count / 5} messages/sec")
    }, 5, 5, TimeUnit.SECONDS)

    logger.info(s"🚀 Starting Ultra-Performance Scala Consumers: $numThreads")

    Runtime.getRuntime.addShutdownHook(new Thread(() => {
      logger.info("🛑 Shutting down Scala consumers...")
      running.set(false)
      reporter.shutdown()
    }))

    val threads = for i <- 0 until numThreads yield
      val threadId = i
      val thread = new Thread(() => {
        val props = new Properties()
        props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
        props.put(ConsumerConfig.GROUP_ID_CONFIG, s"stream-test-result-consumer-scala-$threadId")
        props.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, classOf[ByteArrayDeserializer].getName)
        props.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, classOf[ByteArrayDeserializer].getName)
        props.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "latest")
        props.put(ConsumerConfig.FETCH_MIN_BYTES_CONFIG, "100000")

        val consumer = new KafkaConsumer[Array[Byte], Array[Byte]](props)
        consumer.subscribe(Collections.singletonList(topic))

        logger.info(s"Consumer thread $threadId started")

        try
          while running.get() do
            val records = consumer.poll(Duration.ofMillis(100))
            val it = records.iterator()
            while it.hasNext do
              val record = it.next()
              val payload = record.value()
              if payload != null then
                val in = new ByteArrayInputStream(payload)
                val sum = readVarint(in)
                resultCounter.incrementAndGet()
        catch
          case e: Exception => if (running.get()) logger.error(s"Consumer thread $threadId failed", e)
        finally
          consumer.close()
      })
      thread.start()
      thread

    threads.foreach(_.join())
