FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the caching layer!
RUN cargo chef cook --release --recipe-path recipe.json
# Build application
COPY . .
RUN cargo build --release --bin grpc_server --bin fake_deck_of_cards_api --bin grpc_client

# Runtime base
FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update && apt-get install -y openssl ca-certificates && rm -rf /var/lib/apt/lists/*

# Final stage for grpc_server
FROM runtime AS grpc_server
COPY --from=builder /app/target/release/grpc_server /usr/local/bin/
EXPOSE 25565
CMD ["grpc_server"]

# Final stage for fake_deck_of_cards_api
FROM runtime AS fake_deck_of_cards_api
COPY --from=builder /app/target/release/fake_deck_of_cards_api /usr/local/bin/
EXPOSE 25566
CMD ["fake_deck_of_cards_api"]

# Final stage for grpc_client
FROM runtime AS grpc_client
COPY --from=builder /app/target/release/grpc_client /usr/local/bin/
ENTRYPOINT ["grpc_client"]
