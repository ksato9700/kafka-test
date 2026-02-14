FROM python:3.13-alpine AS builder

WORKDIR /app

# Install build dependencies
RUN apk add --no-cache gcc musl-dev linux-headers

# Install uv
RUN pip install uv

# Copy dependency files
COPY pyproject.toml uv.lock ./

# Install dependencies into a virtual environment
RUN uv sync --frozen

FROM python:3.13-alpine

WORKDIR /app

# Copy the virtual environment from the builder stage
COPY --from=builder /app/.venv /app/.venv

# Set environment variables to use the virtual environment
ENV PATH="/app/.venv/bin:$PATH"

# Copy source code
COPY consumer.py producer.py ./

# Default command
CMD ["python", "consumer.py"]
