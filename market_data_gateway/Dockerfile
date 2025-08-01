FROM rust:1.75 as builder

WORKDIR /app
COPY ./shared-models /app/shared-models
COPY ./market_data_gateway /app/market_data_gateway
RUN cargo build -p market_data_gateway --release

FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/market_data_gateway /usr/local/bin/market_data_gateway
ENTRYPOINT ["market_data_gateway"]
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/market_data_gateway /usr/local/bin/
CMD ["market_data_gateway"]
