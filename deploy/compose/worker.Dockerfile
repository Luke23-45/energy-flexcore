# P1.25 — Worker image: multi-stage build of the flexcore-worker binary.
FROM rust:1.99.1 AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates ./crates
COPY apps ./apps
RUN cargo build --release -p flexcore-worker --bin flexcore-worker

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flexcore-worker /usr/local/bin/flexcore-worker
# Non-root runtime user.
RUN useradd --system --no-create-home flexcore
USER flexcore
EXPOSE 8081
ENTRYPOINT ["/usr/local/bin/flexcore-worker"]
