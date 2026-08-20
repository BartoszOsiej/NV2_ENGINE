FROM rust:1.85-slim AS builder
RUN apt-get update && apt-get install -y libclang-dev libpcap-dev musl-tools && rm -rf /var/lib/apt/lists/*
WORKDIR /build
COPY . .
RUN cargo build --release
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libpcap0.8 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/ /
ENTRYPOINT ["/usr/bin/process-monitor"]
