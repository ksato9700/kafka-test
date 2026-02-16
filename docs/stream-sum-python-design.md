# Design Document: Python Streaming Sum Processor

## 1. Objective
To implement the `streaming-performance-spec.md` in Python. We aim for high throughput by utilizing the C-backed `confluent-kafka` library and the `multiprocessing` module to bypass Python's Global Interpreter Lock (GIL).

## 2. Technology Stack
- **Language**: Python 3.12+
- **Kafka Library**: `confluent-kafka` (built on `librdkafka`).
- **Concurrency**: `multiprocessing` (multiple independent processes to utilize multi-core CPUs).
- **Serialization**: Manual Zig-Zag binary encoding/decoding (matching Java/Rust/Ruby optimizations).

## 3. Optimizations
- **Manual SerDe**: Avoiding the overhead of generic Avro libraries by implementing raw binary encoding/decoding.
- **Multiprocessing**: Each worker runs in its own process, ensuring true parallel execution of the sum logic.
- **C-Extensions**: `confluent-kafka` handles the heavy lifting of Kafka communication in C threads.
- **Buffer Management**: Utilizing `bytearray` and direct indexing where possible.

## 4. Concurrency Model
Python's GIL prevents multiple threads from executing Python bytecode simultaneously. To achieve millions of messages per second, we will spawn multiple processes, each with its own Kafka consumer and producer, allowing the OS to distribute the load across all available CPU cores.
