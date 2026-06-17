FROM ubuntu:22.04

# Avoid interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install build tools, GCC, G++, Make, CMake, Git, Curl, Wget, and Rust toolchain
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    gcc \
    g++ \
    make \
    cmake \
    git \
    curl \
    wget \
    rustc \
    cargo \
    unzip \
    zip \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user for safety
RUN useradd -m -u 1000 builder

# Create workspace directory with proper permissions
RUN mkdir -p /var/code && \
    chown -R builder:builder /var/code && \
    chmod 755 /var/code

# Switch to non-root user
USER builder

# Set working directory
WORKDIR /var/code

# Health check - verify directory exists and is accessible/writable
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD test -d /var/code && test -w /var/code || exit 1

# Keep container running
CMD ["sleep", "infinity"]
