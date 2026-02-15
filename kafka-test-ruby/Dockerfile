FROM ruby:3.3-alpine AS builder

WORKDIR /app

# Install build dependencies
# bash is required by rdkafka gem build script even if using system libs (sometimes)
# but --use-system-libraries usually avoids compiling.
RUN apk add --no-cache build-base openssl-dev librdkafka-dev bash

COPY kafka-test-ruby/Gemfile ./
# Remove system library config to fallback to bundled compilation
# Install gems and cleanup cache/artifacts to reduce image size
RUN bundle config set --global without 'development test' && \
    bundle install && \
    rm -rf /usr/local/bundle/cache/*.gem && \
    find /usr/local/bundle/gems/ -name "*.c" -delete && \
    find /usr/local/bundle/gems/ -name "*.o" -delete && \
    find /usr/local/bundle/gems/rdkafka-*/ext/ -name "tmp" -type d -exec rm -rf {} + && \
    find /usr/local/bundle/gems/rdkafka-*/ext/ -name "ports" -type d -exec rm -rf {} +

FROM ruby:3.3-alpine

WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache librdkafka

COPY --from=builder /usr/local/bundle /usr/local/bundle
COPY schemas /app/schemas
COPY kafka-test-ruby/consumer.rb kafka-test-ruby/producer.rb ./

CMD ["ruby", "consumer.rb"]
