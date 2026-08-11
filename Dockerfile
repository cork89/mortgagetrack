# ---- Build ----
FROM rust:bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
COPY static ./static

RUN cargo build --release

# ---- Runtime ----
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/mortgagetrack /app/mortgagetrack
COPY --from=builder /app/static /app/static

ENV HOST=0.0.0.0
ENV PORT=3000
ENV STATIC_DIR=/app/static
ENV SESSION_SECURE=true
ENV SESSION_SAME_SITE=Lax

EXPOSE 3000
CMD ["/app/mortgagetrack"]
