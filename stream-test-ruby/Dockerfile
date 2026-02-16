FROM ruby:3.3-slim

RUN apt-get update && apt-get install -y 
    build-essential 
    pkg-config 
    libssl-dev 
    zlib1g-dev 
    libzstd-dev 
    libsasl2-dev 
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY stream-test-ruby/Gemfile ./
RUN bundle install

COPY stream-test-ruby/bin ./bin
COPY schemas /app/schemas

CMD ["bundle", "exec", "ruby", "bin/processor.rb"]
