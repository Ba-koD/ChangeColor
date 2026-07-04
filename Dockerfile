FROM rust:1-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
# Limit build parallelism/codegen to keep peak memory low (avoids OOM/SIGKILL on low-RAM Docker VMs)
ENV CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 app

WORKDIR /app
COPY --from=builder /app/target/release/change-color /usr/local/bin/change-color
RUN mkdir -p /app/data && chown -R app:app /app

USER app
ENV DATA_PATH=/app/data/state.json

CMD ["change-color"]
