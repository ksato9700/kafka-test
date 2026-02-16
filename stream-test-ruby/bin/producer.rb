require 'rdkafka'
require 'thread'

$send_counter = 0
$counter_mutex = Mutex.new

def write_varint(n)
  val = (n << 1) ^ (n >> 63)
  res = []
  while val >= 0x80
    res << ((val & 0x7f) | 0x80)
    val >>= 7
  end
  res << val
  res.pack('C*')
end

def main
  bootstrap_servers = ENV.fetch('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
  topic = ENV.fetch('INPUT_TOPIC', 'integer-list-input-avro')
  num_threads = ENV.fetch('PRODUCER_THREADS', '4').to_i

  Thread.new do
    loop do
      sleep 5
      count = 0
      $counter_mutex.synchronize do
        count = $send_counter
        $send_counter = 0
      end
      puts "📤 Ruby Sending Throughput: #{count / 5} messages/sec" if count > 0
    end
  end

  puts "🚀 Starting Ultra-Performance Ruby Producer with #{num_threads} threads..."

  threads = num_threads.times.map do |i|
    Thread.new do
      config = {
        "bootstrap.servers": bootstrap_servers,
        "broker.address.family": "v4",
        "compression.codec": "snappy",
        "batch.num.messages": 100000,
        "linger.ms": 20,
        "queue.buffering.max.messages": 1000000
      }
      producer = Rdkafka::Config.new(config).producer
      puts "Producer thread #{i} started"
      
      local_counter = 0
      loop do
        count = rand(2..5)
        buf = write_varint(count)
        count.times { buf << write_varint(rand(0..99)) }
        buf << write_varint(0)

        producer.produce(topic: topic, payload: buf)
        
        local_counter += 1
        if local_counter >= 1000
          $counter_mutex.synchronize { $send_counter += local_counter }
          local_counter = 0
        end
      end
    end
  end

  threads.each(&:join)
end

main
