FROM rust:1.89-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p sealforge-coordinator

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/sealforge-coordinator /usr/local/bin/sealforge-coordinator
USER 10001
EXPOSE 8080
ENTRYPOINT ["sealforge-coordinator"]
