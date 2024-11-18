# Build stage
FROM rust:1.82.0-slim-bullseye as builder

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev protobuf-compiler postgresql-client && \
    rm -rf /var/lib/apt/lists/*

# Create a non-root user for sqlx
RUN useradd -m rust

WORKDIR /usr/src/ent

# Install sqlx-cli globally for all users
RUN cargo install sqlx-cli --no-default-features --features postgres

# Copy only dependency files first to cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY proto/ proto/

# Create a dummy main.rs to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src/

# Copy the rest of the source code
COPY . .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y ca-certificates libssl1.1 postgresql-client && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary and migrations
COPY --from=builder /usr/src/ent/target/release/ent /app/
COPY --from=builder /usr/src/ent/migrations /app/migrations/
# Copy sqlx from the correct location
COPY --from=builder /usr/local/cargo/bin/sqlx /usr/local/bin/
COPY --from=builder /usr/src/ent/config /app/config/

# Copy startup script
COPY scripts/start.sh /app/
RUN chmod +x /app/start.sh

# Run as non-root user
RUN useradd -m ent && chown -R ent:ent /app
USER ent

ENTRYPOINT ["/app/start.sh"]
CMD ["./ent"]
