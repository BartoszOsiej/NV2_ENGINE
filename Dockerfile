FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y libclang-dev libx11-dev libxi-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /build/Core
COPY . /build
RUN cargo build --release
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libx11-6 libxkbcommon0 libwayland-client0 libgl1 mesa-utils && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/Core/target/release/ /usr/bin/
ENTRYPOINT ["/usr/bin/nv2_engine"]
