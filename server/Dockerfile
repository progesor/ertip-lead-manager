FROM rust:bookworm AS builder

WORKDIR /build

COPY server/Cargo.toml ./Cargo.toml
COPY server/migrations ./migrations
COPY server/src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 appuser

WORKDIR /app

COPY --from=builder /build/target/release/ertip-lead-manager-server /usr/local/bin/ertip-lead-manager-server

ENV ELM_BIND_ADDR=0.0.0.0:8080
ENV RUST_LOG=info,ertip_lead_manager_server=info,tower_http=info

EXPOSE 8080

USER appuser

ENTRYPOINT ["/usr/local/bin/ertip-lead-manager-server"]
