FROM python:3.13-alpine AS builder

WORKDIR /app

# Install build dependencies
RUN apk add --no-cache gcc musl-dev linux-headers

# Install uv
RUN pip install uv

# Copy dependency files (relative to root context)
COPY kafka-test-python/pyproject.toml kafka-test-python/uv.lock ./

# Install dependencies into a virtual environment
RUN uv sync --frozen

FROM python:3.13-alpine

WORKDIR /app

# Copy the virtual environment from the builder stage
COPY --from=builder /app/.venv /app/.venv

# Set environment variables to use the virtual environment
ENV PATH="/app/.venv/bin:$PATH"

# Copy schemas
COPY schemas /app/schemas

# Copy source code
COPY kafka-test-python/consumer.py kafka-test-python/producer.py ./

# Default command
CMD ["python", "consumer.py"]
