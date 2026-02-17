CC=gcc
CFLAGS=-O3 -Wall -I/opt/homebrew/include
LDFLAGS=-L/opt/homebrew/lib -lrdkafka -lpthread

all: stream-test-c

stream-test-c: src/main.c
	$(CC) $(CFLAGS) -o stream-test-c src/main.c $(LDFLAGS)

run-benchmark: stream-test-c
	BENCHMARK=1 NUM_WORKERS=8 ./stream-test-c

clean:
	rm -f stream-test-c
