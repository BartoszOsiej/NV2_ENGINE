FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y libclang-dev libpcap-dev pkg-config libx11-dev libxi-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libpcap0.8 libx11-6 libxkbcommon0 libwayland-client0 libgl1 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/ /usr/bin/
ENTRYPOINT ["/usr/bin/nv2_engine"]
