# P1.25 — Control-plane image: multi-stage build of the flexcore-control-plane
# binary. dev-auth is NOT enabled (release builds deny unauthenticated
# requests; see apps/control-plane/src/auth.rs for the recorded Phase 1
# authentication status).
FROM rust:1.99.1 AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates ./crates
COPY apps ./apps
RUN cargo build --release -p flexcore-control-plane --bin flexcore-control-plane

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flexcore-control-plane /usr/local/bin/flexcore-control-plane
# Non-root runtime user.
RUN useradd --system --no-create-home flexcore
USER flexcore
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/flexcore-control-plane"]
