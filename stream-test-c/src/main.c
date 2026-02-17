#include <stdio.h>
#include <signal.h>
#include <string.h>
#include <stdlib.h>
#include <time.h>
#include <pthread.h>
#include <stdatomic.h>
#include <unistd.h>
#include <librdkafka/rdkafka.h>

#define TOTAL_RECORDS 50000000L
#define INPUT_TOPIC "integer-list-input-benchmark"
#define OUTPUT_TOPIC "integer-sum-output-benchmark"

atomic_long processed_counter = 0;
atomic_int start_trigger = 0;
struct timespec start_time;

static int64_t read_varint(const uint8_t **data, const uint8_t *end) {
    uint64_t val = 0;
    int shift = 0;
    while (*data < end) {
        uint8_t b = **data;
        (*data)++;
        val |= (uint64_t)(b & 0x7F) << shift;
        if (!(b & 0x80)) break;
        shift += 7;
    }
    return (int64_t)(val >> 1) ^ -(int64_t)(val & 1);
}

static int write_varint(uint8_t *buf, int64_t n) {
    uint64_t val = (uint64_t)((n << 1) ^ (n >> 63));
    int len = 0;
    while (val >= 0x80) {
        buf[len++] = (uint8_t)(val | 0x80);
        val >>= 7;
    }
    buf[len++] = (uint8_t)val;
    return len;
}

typedef struct {
    char *brokers;
    char *group_id;
    int id;
} thread_args_t;

void *worker_thread(void *arg) {
    thread_args_t *args = (thread_args_t *)arg;
    char errstr[512];

    rd_kafka_conf_t *conf = rd_kafka_conf_new();
    rd_kafka_conf_set(conf, "bootstrap.servers", args->brokers, NULL, 0);
    rd_kafka_conf_set(conf, "group.id", args->group_id, NULL, 0);
    rd_kafka_conf_set(conf, "auto.offset.reset", "earliest", NULL, 0);
    rd_kafka_conf_set(conf, "broker.address.family", "v4", NULL, 0);
    // Performance Tuning
    rd_kafka_conf_set(conf, "fetch.min.bytes", "1000000", NULL, 0);
    rd_kafka_conf_set(conf, "fetch.message.max.bytes", "10000000", NULL, 0);
    rd_kafka_conf_set(conf, "queued.min.messages", "2000000", NULL, 0);

    rd_kafka_t *rk_c = rd_kafka_new(RD_KAFKA_CONSUMER, conf, errstr, sizeof(errstr));
    rd_kafka_poll_set_consumer(rk_c);
    rd_kafka_topic_partition_list_t *topics = rd_kafka_topic_partition_list_new(1);
    rd_kafka_topic_partition_list_add(topics, INPUT_TOPIC, RD_KAFKA_PARTITION_UA);
    rd_kafka_subscribe(rk_c, topics);
    rd_kafka_topic_partition_list_destroy(topics);

    rd_kafka_conf_t *p_conf = rd_kafka_conf_new();
    rd_kafka_conf_set(p_conf, "bootstrap.servers", args->brokers, NULL, 0);
    rd_kafka_conf_set(p_conf, "compression.type", "snappy", NULL, 0);
    rd_kafka_conf_set(p_conf, "batch.num.messages", "100000", NULL, 0);
    rd_kafka_conf_set(p_conf, "broker.address.family", "v4", NULL, 0);
    rd_kafka_t *rk_p = rd_kafka_new(RD_KAFKA_PRODUCER, p_conf, errstr, sizeof(errstr));

    printf("Worker %d started\n", args->id);

    uint8_t out_buf[32];
    while (atomic_load(&processed_counter) < TOTAL_RECORDS) {
        rd_kafka_message_t *m = rd_kafka_consumer_poll(rk_c, 10);
        if (!m) continue;
        if (m->err) { rd_kafka_message_destroy(m); continue; }

        if (atomic_exchange(&start_trigger, 1) == 0) clock_gettime(CLOCK_MONOTONIC, &start_time);

        const uint8_t *payload = (const uint8_t *)m->payload;
        const uint8_t *end = payload + m->len;
        int64_t count = read_varint(&payload, end);
        int64_t sum = 0;
        while (count != 0) {
            int64_t abs_count = (count < 0) ? -count : count;
            if (count < 0) read_varint(&payload, end);
            for (int j = 0; j < abs_count; j++) sum += read_varint(&payload, end);
            count = read_varint(&payload, end);
        }

        int out_len = write_varint(out_buf, sum);
        rd_kafka_producev(rk_p, RD_KAFKA_V_TOPIC(OUTPUT_TOPIC), RD_KAFKA_V_MSGFLAGS(RD_KAFKA_MSG_F_COPY),
                          RD_KAFKA_V_VALUE(out_buf, out_len), RD_KAFKA_V_KEY(m->key, m->key_len), RD_KAFKA_V_END);

        atomic_fetch_add(&processed_counter, 1);
        rd_kafka_message_destroy(m);
        if (atomic_load(&processed_counter) % 1000 == 0) rd_kafka_poll(rk_p, 0);
    }

    rd_kafka_flush(rk_p, 10000);
    rd_kafka_destroy(rk_p);
    rd_kafka_consumer_close(rk_c);
    rd_kafka_destroy(rk_c);
    return NULL;
}

int main(int argc, char **argv) {
    char *brokers = getenv("KAFKA_BOOTSTRAP_SERVERS");
    if (!brokers) brokers = "127.0.0.1:9094";
    int num_workers = 8;
    char *nw = getenv("NUM_WORKERS");
    if (nw) num_workers = atoi(nw);

    if (getenv("BENCHMARK")) {
        printf("🚀 Phase 1: Loading %ld records (C)...\n", TOTAL_RECORDS);
        rd_kafka_conf_t *conf = rd_kafka_conf_new();
        rd_kafka_conf_set(conf, "bootstrap.servers", brokers, NULL, 0);
        rd_kafka_conf_set(conf, "compression.type", "snappy", NULL, 0);
        rd_kafka_conf_set(conf, "broker.address.family", "v4", NULL, 0);
        char errstr[512];
        rd_kafka_t *rk = rd_kafka_new(RD_KAFKA_PRODUCER, conf, errstr, sizeof(errstr));
        uint8_t buf[64];
        for (long i = 0; i < TOTAL_RECORDS; i++) {
            int count = (rand() % 4) + 2;
            int pos = write_varint(buf, count);
            for (int j = 0; j < count; j++) pos += write_varint(buf + pos, rand() % 100);
            pos += write_varint(buf + pos, 0);
            rd_kafka_producev(rk, RD_KAFKA_V_TOPIC(INPUT_TOPIC), RD_KAFKA_V_MSGFLAGS(RD_KAFKA_MSG_F_COPY),
                              RD_KAFKA_V_VALUE(buf, pos), RD_KAFKA_V_END);
            if (i % 100000 == 0) rd_kafka_poll(rk, 0);
        }
        rd_kafka_flush(rk, 60000);
        rd_kafka_destroy(rk);
        printf("✅ Phase 1 Complete.\n");
    }

    printf("🚀 Phase 2: Processing backlog with %d workers...\n", num_workers);
    char group_id[64]; snprintf(group_id, sizeof(group_id), "benchmark-c-%ld", time(NULL));
    pthread_t *threads = malloc(num_workers * sizeof(pthread_t));
    thread_args_t *t_args = malloc(num_workers * sizeof(thread_args_t));

    for (int i = 0; i < num_workers; i++) {
        t_args[i].brokers = brokers; t_args[i].group_id = group_id; t_args[i].id = i;
        pthread_create(&threads[i], NULL, worker_thread, &t_args[i]);
    }

    while (atomic_load(&processed_counter) < TOTAL_RECORDS) {
        long c = atomic_load(&processed_counter);
        if (c > 0) printf("\rProgress: %ld%% (%ld / %ld)", (c * 100) / TOTAL_RECORDS, c, TOTAL_RECORDS);
        fflush(stdout); sleep(1);
    }

    for (int i = 0; i < num_workers; i++) pthread_join(threads[i], NULL);
    struct timespec end_time; clock_gettime(CLOCK_MONOTONIC, &end_time);
    double total_time = (end_time.tv_sec - start_time.tv_sec) + (end_time.tv_nsec - start_time.tv_nsec) / 1e9;
    printf("\n🏁 BENCHMARK RESULT (C) 🏁\n");
    printf("Total Records: %ld\n", TOTAL_RECORDS);
    printf("Processing Time: %.2fs\n", total_time);
    printf("Throughput:      %.0f msg/sec\n", TOTAL_RECORDS / total_time);
    free(threads); free(t_args);
    return 0;
}
