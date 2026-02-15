require "rdkafka"
require "json"
require "time"
require "logger"

def main
  STDOUT.sync = true
  logger = Logger.new(STDOUT)
  logger.formatter = proc do |severity, datetime, progname, msg|
    "#{datetime.strftime('%Y-%m-%d %H:%M:%S')} #{severity} #{msg}\n"
  end

  logger.info "Hello from kafka-test-ruby!"

  bootstrap_servers = ENV.fetch("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
  topic = ENV.fetch("TOPIC_NAME", "my-topic-1")

  config = {
    "bootstrap.servers": bootstrap_servers,
    "broker.address.family": "v4",
    "message.timeout.ms": 5000
  }

  producer = Rdkafka::Config.new(config).producer

  logger.info "🚀 Producer is now running..."

  message_id = 0
  loop do
    event_time = Time.now.to_f
    message = {
      message_id: message_id,
      event_time: event_time,
      content: "Message #{message_id}"
    }

    payload = JSON.generate(message)

    begin
      producer.produce(
          topic: topic,
          payload: payload
      ).wait

      logger.info "🚀 Sent: #{payload}"
    rescue Rdkafka::RdkafkaError => e
      logger.error "❌ Error sending message: #{e.message}"
    end

    message_id += 1
    sleep 2
  end
rescue Interrupt
  logger.info "🛑 Shutting down producer..."
ensure
  if defined?(producer) && producer
    producer.close
    logger.info "✅ Producer closed."
  end
end

main if __FILE__ == $0
