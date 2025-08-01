FROM rust:1.75 as builder
WORKDIR /app
COPY ./shared-models /app/shared-models
COPY ./portfolio_manager /app/portfolio_manager
RUN cargo build -p portfolio_manager --release

FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/portfolio_manager /usr/local/bin/portfolio_manager
ENTRYPOINT ["portfolio_manager"]
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/portfolio_manager /usr/local/bin/
CMD ["portfolio_manager"]
