FROM rust:1.75 as builder
WORKDIR /app
COPY ./shared-models /app/shared-models
COPY ./strategy_factory /app/strategy_factory
RUN cargo build -p strategy_factory --release

FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/strategy_factory /usr/local/bin/strategy_factory
ENTRYPOINT ["strategy_factory"]
