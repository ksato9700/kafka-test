require 'rdkafka'
require 'stringio'

def read_varint(io)
  val = 0
  shift = 0
  loop do
    b = io.getbyte
    return 0 unless b
    val |= (b & 0x7f) << shift
    break if (b & 0x80) == 0
    shift += 7
  end
  (val >> 1) ^ -(val & 1)
end

def main
  bootstrap_servers = ENV.fetch('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
  topic = ENV.fetch('OUTPUT_TOPIC', 'integer-sum-output-avro')

  config = {
    "bootstrap.servers": bootstrap_servers,
    "broker.address.family": "v4",
    "group.id": "ruby-test-consumer-group",
    "auto.offset.reset": "earliest"
  }

  consumer = Rdkafka::Config.new(config).consumer
  consumer.subscribe(topic)

  puts "🚀 Waiting for Avro results on topic: #{topic}..."

  loop do
    message = consumer.poll(100)
    next unless message

    io = StringIO.new(message.payload)
    sum = read_varint(io)
    puts "Received Avro Result: sum=#{sum}"
  end
end

main
