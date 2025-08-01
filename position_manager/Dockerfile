FROM rust:1.75 as builder

WORKDIR /app
COPY ./shared-models /app/shared-models
COPY ./position_manager /app/position_manager
RUN cargo build -p position_manager --release

FROM gcr.io/distroless/cc
COPY --from=builder /app/target/release/position_manager /usr/local/bin/position_manager
ENTRYPOINT ["position_manager"]
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/position_manager /usr/local/bin/
CMD ["position_manager"]
