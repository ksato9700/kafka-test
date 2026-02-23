# Benchmark Summary (50 Million Records, 8 Workers)

| Language | Best Processing Time | Best Throughput |
| :--- | :--- | :--- |
| **Java** | 7.92 seconds | 6,312,468 msg/sec |
| **C** | 10.59 seconds | 4,719,502 msg/sec |
| **Rust** | 14.29 seconds | 3,499,934 msg/sec |

## Key Observations
*   **Java** achieved the highest peak performance, reaching over 6.3 million messages per second.
*   **C** maintained consistent performance, averaging around 4.5 million messages per second.
*   **Rust** peaked at approximately 3.5 million messages per second in these specific tests.
*   All tests were performed on a backlog of 50,000,000 records.
