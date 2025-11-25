# syntax=docker/dockerfile:1.6

FROM rustlang/rust:nightly-slim AS base
WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef --locked

FROM base AS planner
COPY Cargo.toml Cargo.lock ./
COPY backtest_engine ./backtest_engine
COPY executor ./executor
COPY market_data_gateway ./market_data_gateway
COPY portfolio_manager ./portfolio_manager
COPY position_manager ./position_manager
COPY shared-models ./shared-models
COPY signer ./signer
COPY strategy_factory ./strategy_factory
COPY tests ./tests
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --locked --package executor --bin executor \
    && cp /app/target/release/executor /app/executor_bin

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/executor_bin /usr/local/bin/executor
EXPOSE 9184
ENTRYPOINT ["/usr/local/bin/executor"]
