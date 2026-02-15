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

  bootstrap_servers = ENV.fetch("KAFKA_BOOTSTRAP_SERVERS", "127.0.0.1:9094")
  topic = ENV.fetch("TOPIC_NAME", "my-topic-1")
  group_id = ENV.fetch("CONSUMER_GROUP_ID", "my-group-1")

  logger.info "🛠️ Connecting KafkaConsumer to topic '#{topic}' at '#{bootstrap_servers}' (group: '#{group_id}')..."

  config = {
    "bootstrap.servers": bootstrap_servers,
    "group.id": group_id,
    "auto.offset.reset": "latest",
    "enable.auto.commit": true,
    "broker.address.family": "v4"
  }

  consumer = Rdkafka::Config.new(config).consumer
  consumer.subscribe(topic)

  logger.info "✅ KafkaConsumer connected. Waiting for new messages..."
  logger.info "📥 Listening for messages..."

  consumer.each do |message|
    begin
      data = JSON.parse(message.payload)
      received_time = Time.now.to_f

      event_time = data["event_time"] || received_time
      latency = received_time - event_time

      logger.info "📨 New message: [ID=#{data['message_id']}] #{data['content']} at #{event_time.round(3)}"
      logger.info "⏱️ Latency: #{latency.round(3)} seconds"
    rescue JSON::ParserError => e
      logger.warn "⚠️ JSON parsing error: #{e.message} - payload: #{message.payload}"
    rescue => e
      logger.error "⚠️ Unexpected error: #{e.class} - #{e.message}"
    end
  end
rescue Interrupt
  logger.info "🛑 Shutting down gracefully..."
ensure
  if defined?(consumer) && consumer
    consumer.close
    logger.info "✅ KafkaConsumer closed."
  end
end

main if __FILE__ == $0
