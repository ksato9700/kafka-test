package com.example.kafka

import org.apache.kafka.clients.admin.{AdminClient, NewTopic}
import org.apache.kafka.clients.consumer.{ConsumerConfig, KafkaConsumer}
import org.apache.kafka.clients.producer.{KafkaProducer, ProducerConfig, ProducerRecord}
import org.apache.kafka.common.serialization.{ByteArrayDeserializer, ByteArraySerializer}
import org.slf4j.{Logger, LoggerFactory}

import java.io.{ByteArrayInputStream, ByteArrayOutputStream}
import java.time.Duration
import java.util.{Arrays, Collections, Properties}
import java.util.concurrent.atomic.{AtomicBoolean, AtomicLong}
import java.util.concurrent.{Executors, TimeUnit}
import scala.jdk.CollectionConverters.*

object StreamSumApp:
  private val logger: Logger = LoggerFactory.getLogger(getClass)
  private val messageCounter = new AtomicLong(0)
  private val running = new AtomicBoolean(true)

  private def createTopics(bootstrapServers: String, topics: List[String]): Unit =
    val props = new Properties()
    props.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
    val adminClient = AdminClient.create(props)
    try
      for topic <- topics do
        val newTopic = new NewTopic(topic, 12, 1.toShort)
        adminClient.createTopics(Collections.singletonList(newTopic)).all().get(2, TimeUnit.SECONDS)
    catch
      case _: Exception => // Ignore
    finally
      adminClient.close()

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

  private def writeVarint(out: ByteArrayOutputStream, n: Long): Unit =
    var val_res = (n << 1) ^ (n >> 63)
    while (val_res >= 0x80) {
      out.write((val_res | 0x80).toInt)
      val_res >>>= 7
    }
    out.write(val_res.toInt)

  def main(args: Array[String]): Unit =
    val bootstrapServers = sys.env.getOrElse("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
    val inputTopic = sys.env.getOrElse("INPUT_TOPIC", "integer-list-input-avro")
    val outputTopic = sys.env.getOrElse("OUTPUT_TOPIC", "integer-sum-output-avro")
    val groupId = sys.env.getOrElse("APPLICATION_ID", "stream-sum-test-scala-avro")
    val numWorkers = sys.env.getOrElse("STREAMS_NUM_STREAM_THREADS", "8").toInt

    createTopics(bootstrapServers, List(inputTopic, outputTopic))

    val reporter = Executors.newSingleThreadScheduledExecutor()
    reporter.scheduleAtFixedRate(() => {
      val count = messageCounter.getAndSet(0)
      if (count > 0) logger.info(s"🚀 Scala Throughput: ${count / 5} messages/sec")
    }, 5, 5, TimeUnit.SECONDS)

    logger.info(s"🚀 Starting Ultra-Performance Scala Workers: $numWorkers")

    Runtime.getRuntime.addShutdownHook(new Thread(() => {
      logger.info("🛑 Shutting down Scala stream processor...")
      running.set(false)
      reporter.shutdown()
    }))

    val threads = for i <- 0 until numWorkers yield
      val workerId = i
      val thread = new Thread(() => {
        val prodProps = new Properties()
        prodProps.put(ProducerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
        prodProps.put(ProducerConfig.KEY_SERIALIZER_CLASS_CONFIG, classOf[ByteArraySerializer].getName)
        prodProps.put(ProducerConfig.VALUE_SERIALIZER_CLASS_CONFIG, classOf[ByteArraySerializer].getName)
        prodProps.put(ProducerConfig.ACKS_CONFIG, "1")
        prodProps.put(ProducerConfig.BATCH_SIZE_CONFIG, "131072")
        prodProps.put(ProducerConfig.LINGER_MS_CONFIG, "20")
        prodProps.put(ProducerConfig.COMPRESSION_TYPE_CONFIG, "snappy")
        val producer = new KafkaProducer[Array[Byte], Array[Byte]](prodProps)

        val consProps = new Properties()
        consProps.put(ConsumerConfig.BOOTSTRAP_SERVERS_CONFIG, bootstrapServers)
        consProps.put(ConsumerConfig.GROUP_ID_CONFIG, groupId)
        consProps.put(ConsumerConfig.KEY_DESERIALIZER_CLASS_CONFIG, classOf[ByteArrayDeserializer].getName)
        consProps.put(ConsumerConfig.VALUE_DESERIALIZER_CLASS_CONFIG, classOf[ByteArrayDeserializer].getName)
        consProps.put(ConsumerConfig.AUTO_OFFSET_RESET_CONFIG, "earliest")
        consProps.put(ConsumerConfig.FETCH_MIN_BYTES_CONFIG, "100000")
        consProps.put(ConsumerConfig.FETCH_MAX_WAIT_MS_CONFIG, "100")
        val consumer = new KafkaConsumer[Array[Byte], Array[Byte]](consProps)
        consumer.subscribe(Collections.singletonList(inputTopic))

        val outBuf = new ByteArrayOutputStream(64)
        var localCounter = 0L

        logger.info(s"Worker thread $workerId started")

        try
          while running.get() do
            val records = consumer.poll(Duration.ofMillis(100))
            val it = records.iterator()
            while it.hasNext do
              val record = it.next()
              val payload = record.value()
              if payload != null then
                val in = new ByteArrayInputStream(payload)
                var sum = 0L
                var count = readVarint(in)
                while count != 0 do
                  val absCount = Math.abs(count)
                  if count < 0 then readVarint(in) // skip byte size
                  var j = 0
                  while j < absCount do
                    sum += readVarint(in)
                    j += 1
                  count = readVarint(in)

                outBuf.reset()
                writeVarint(outBuf, sum)
                producer.send(new ProducerRecord(outputTopic, record.key(), outBuf.toByteArray))

                localCounter += 1
                if localCounter >= 1000 then
                  messageCounter.addAndGet(localCounter)
                  localCounter = 0
        catch
          case e: Exception => if (running.get()) logger.error(s"Worker $workerId failed", e)
        finally
          producer.close()
          consumer.close()
      })
      thread.setName(s"worker-$i")
      thread.start()
      thread

    threads.foreach(_.join())
