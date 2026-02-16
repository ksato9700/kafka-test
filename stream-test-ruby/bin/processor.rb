require 'rdkafka'

def main
  bootstrap_servers = ENV.fetch('KAFKA_BOOTSTRAP_SERVERS', '127.0.0.1:9094')
  input_topic = ENV.fetch('INPUT_TOPIC', 'integer-list-input-avro')
  output_topic = ENV.fetch('OUTPUT_TOPIC', 'integer-sum-output-avro')
  group_id = ENV.fetch('APPLICATION_ID', 'stream-sum-test-ruby-avro')
  num_workers = ENV.fetch('NUM_WORKERS', '4').to_i

  puts "🚀 Starting Ultra-Optimized Multi-Process Ruby Stream Processor..."

  num_workers.times do |i|
    Process.fork do
      producer = Rdkafka::Config.new({
        "bootstrap.servers": bootstrap_servers,
        "broker.address.family": "v4",
        "compression.codec": "snappy",
        "batch.num.messages": 100000,
        "linger.ms": 20
      }).producer

      consumer = Rdkafka::Config.new({
        "bootstrap.servers": bootstrap_servers,
        "broker.address.family": "v4",
        "group.id": group_id,
        "auto.offset.reset": "earliest",
        "enable.auto.commit": true,
        "fetch.min.bytes": 100000
      }).consumer
      consumer.subscribe(input_topic)

      local_counter = 0
      last_report = Time.now

      loop do
        message = consumer.poll(100)
        next unless message

        payload = message.payload
        pos = 0
        sum = 0
        
        # Inline Varint Decode for array count
        v_c = 0; s_c = 0
        loop do
          b = payload.getbyte(pos); pos += 1
          v_c |= (b & 0x7f) << s_c
          break if (b & 0x80) == 0; s_c += 7
        end
        count = (v_c >> 1) ^ -(v_c & 1)

        while count != 0
          abs_count = count.abs
          if count < 0
            v_s = 0; s_s = 0
            loop do
              b = payload.getbyte(pos); pos += 1
              v_s |= (b & 0x7f) << s_s
              break if (b & 0x80) == 0; s_s += 7
            end
          end
          
          abs_count.times do
            v_e = 0; s_e = 0
            loop do
              b = payload.getbyte(pos); pos += 1
              v_e |= (b & 0x7f) << s_e
              break if (b & 0x80) == 0; s_e += 7
            end
            sum += (v_e >> 1) ^ -(v_e & 1)
          end
          
          v_n = 0; s_n = 0
          loop do
            b = payload.getbyte(pos); pos += 1
            v_n |= (b & 0x7f) << s_n
            break if (b & 0x80) == 0; s_n += 7
          end
          count = (v_n >> 1) ^ -(v_n & 1)
        end

        # Inline Varint Encode
        v_enc = (sum << 1) ^ (sum >> 63)
        if v_enc < 0x80
          res_bin = [v_enc].pack('C')
        else
          res_bytes = []
          while v_enc >= 0x80
            res_bytes << ((v_enc & 0x7f) | 0x80)
            v_enc >>= 7
          end
          res_bytes << v_enc
          res_bin = res_bytes.pack('C*')
        end

        producer.produce(topic: output_topic, payload: res_bin, key: message.key)

        local_counter += 1
        if local_counter >= 10000
          now = Time.now
          elap = now - last_report
          if elap >= 5
            puts "🚀 PID #{Process.pid} Throughput: #{(local_counter / elap).to_i} messages/sec"
            local_counter = 0
            last_report = now
          end
        end
      end
    end
  end

  Process.waitall
end

main
