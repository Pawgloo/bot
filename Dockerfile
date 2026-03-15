# ── Build stage ──────────────────────────────────────────────
FROM rust:1.86-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies by copying manifests first
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --release && rm -rf src

# Build the real application
COPY src ./src
RUN touch src/main.rs && cargo build --release

# ── Runtime stage ────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

# Run as non-root user for security
RUN useradd --create-home appuser
USER appuser

WORKDIR /home/appuser

COPY --from=builder /app/target/release/bot ./bot

EXPOSE 3000

LABEL org.opencontainers.image.source="https://github.com/Pawgloo/bot"
LABEL org.opencontainers.image.description="Pawgloo GitHub App — AI PR Reviewer"

CMD ["./bot"]
