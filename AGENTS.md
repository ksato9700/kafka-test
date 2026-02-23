# Instructions for AI Agents

This document provides context and guidelines for AI agents (like Gemini, ChatGPT, Claude) working on this repository.

## 🎯 Project Goal
To maintain a set of **canonical, interoperable Kafka client implementations** across various programming languages. Each implementation must behave strictly identically to the others to serve as a reference architecture.

## 🧠 Core Design Principles

When modifying code or adding a new language, adhere to these principles:

1.  **Parity is Paramount:**
    -   Log messages must match the format: `📨 New message: [ID=...] ...` and `⏱️ Latency: ...`.
    -   Configuration defaults must match (Bootstrap: `127.0.0.1:9094` local, `kafka-broker:9092` Docker).
    -   Error handling (SIGINT) must be implemented to ensure offset commits.

2.  **Infrastructure Awareness:**
    -   The project uses a **Dual Listener** setup.
    -   **Docker:** Clients run on `kafka-net` network and connect to `kafka-broker:9092`.
    -   **Local:** Clients run on host and connect to `127.0.0.1:9094`.
    -   **IPv6:** Always force IPv4 (`java.net.preferIPv4Stack`, `broker.address.family`, or socket patches) to avoid `localhost` resolution lag/errors on macOS.

3.  **Build System:**
    -   Every language directory must have a `Makefile` with standard targets:
        -   `run-consumer`, `run-producer` (Local)
        -   `docker-build`, `docker-run-consumer`, `docker-run-producer` (Docker)
    -   Dockerfiles should be multi-stage and Alpine-based where possible to minimize size.

## 📋 Checklist for Adding a New Language

If you are asked to add a new language (e.g., Go, Node.js, C#):

- [ ] Create directory `kafka-test-<lang>`.
- [ ] Implement `Producer` and `Consumer` following `docs/kafka-client-design.md`.
- [ ] **Crucial:** Implement latency calculation (`now - event_time`). Ensure `event_time` is sent/parsed as **seconds** (double), not milliseconds.
- [ ] **Crucial:** Implement graceful shutdown (catch SIGINT/SIGTERM) to close the client and commit offsets.
- [ ] Create a `Dockerfile` (Multi-stage, Alpine).
- [ ] Create a `Makefile` with standard targets.
- [ ] Add `.dockerignore` and `.gitignore`.
- [ ] Verify local execution against `localhost:9094`.
- [ ] Verify Docker execution against `kafka-broker:9092` using `kafka-net`.

## ⚡ High-Performance Benchmark Guidelines (`stream-test-*`)

The benchmark implementations have stricter performance requirements and use a different methodology:

1.  **Zero-Allocation SerDe:**
    -   Use manual **Zig-Zag binary encoding** (LEB128) instead of generic Avro libraries to eliminate reflection and allocation overhead.
    -   Reuse buffers across messages where possible.
2.  **Concurrency Model:**
    -   Use multiple worker threads/processes, each with its own isolated Kafka Consumer and Producer.
    -   Avoid shared state or global locks in the hot path.
3.  **Batch Benchmark Mode:**
    -   Implement a "Load Phase" that pre-fills a topic with 50M records.
    -   Implement a "Process Phase" that stops once the target count is reached.
    -   Must handle `kafka.ErrQueueFull` (Go) or `BufferError` (Python) with efficient polling to avoid deadlocks.

## 🐛 Common Pitfalls to Avoid

-   **Milliseconds vs Seconds:** Ensure timestamp math is consistent.
-   **Output Buffering:** In Docker, ensure stdout is flushed immediately (e.g., `python -u`, `STDOUT.sync = true`).
-   **Graceful Shutdown in Docker:** Ensure the `Dockerfile` `CMD` or `ENTRYPOINT` allows signals to propagate to the application (exec form `CMD ["app"]` vs shell form `CMD app`).
-   **Single-Node Broker:** Remember that the broker requires `KAFKA_OFFSETS_TOPIC_REPLICATION_FACTOR=1`. This is handled in `run-kafka.sh`, but clients should be robust to temporary connection failures.
