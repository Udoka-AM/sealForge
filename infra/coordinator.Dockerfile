# sealForge coordinator. The build stage needs clang, libsqlite3-dev,
# protobuf-compiler and pkg-config, which the rust image lacks; this is the
# file the hosted testnet site is built from (docs/DEPLOY-VPS.md).
FROM rust:1.99-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config clang libsqlite3-dev protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p sealforge-coordinator

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/sealforge-coordinator /usr/local/bin/sealforge-coordinator
USER 10001
EXPOSE 8080
ENTRYPOINT ["sealforge-coordinator"]
