FROM python:3.12-slim

COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /bin/

RUN apt-get update && apt-get install -y \
    build-essential \
    librdkafka-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Enable bytecode compilation
ENV UV_COMPILE_BYTECODE=1
# Copy only necessary files for dependency installation
COPY stream-test-python/pyproject.toml ./

# Install dependencies
RUN uv sync --no-install-project

COPY stream-test-python/bin ./bin
COPY schemas /app/schemas

CMD ["uv", "run", "bin/processor.py"]
