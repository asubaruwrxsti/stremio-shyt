# Use the official Rust image as a base
FROM rust:latest AS builder

# Set the working directory in the container
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Remove the conflicting package section and dependencies from root Cargo.toml for workspace build
RUN sed -i '/^\[package\]/,/^$/d; /^\[dependencies\]/,/^$/d' Cargo.toml

# Install diesel CLI
RUN cargo install diesel_cli --no-default-features --features sqlite

# Build the application
RUN cargo build --release -p api-server

# Use a smaller base image for the final container
FROM debian:bookworm-slim

# Install necessary system dependencies including curl for health checks
RUN apt-get update && apt-get install -y \
	ca-certificates \
	libssl3 \
	libsqlite3-0 \
	curl \
	&& rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -r -s /bin/false stremio

# Set the working directory
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /app/target/release/api-server /app/api-server

# Copy diesel CLI from builder stage
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/bin/diesel

# Copy entrypoint script
COPY docker/entrypoint.sh /app/entrypoint.sh

# Make entrypoint executable
RUN chmod +x /app/entrypoint.sh

# Create directories for database and downloads and set ownership
RUN mkdir -p /app/data /app/downloads && chown -R stremio:stremio /app

# Switch to non-root user
USER stremio

# Expose the port the app runs on
EXPOSE 8080

# Set environment variables with defaults
ENV RUST_LOG=info
ENV DATABASE_PATH=/app/data/stremio.db
ENV API_HOST=0.0.0.0
ENV API_PORT=8080
ENV DOWNLOAD_DIR=/app/downloads

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
	CMD curl -f http://localhost:8080/health || exit 1

# Run the application
CMD ["./entrypoint.sh"]
