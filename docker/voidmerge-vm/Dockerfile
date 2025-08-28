FROM rust:1.89.0-slim-trixie AS builder

WORKDIR /usr/src/vm

ADD ["Cargo.toml", "Cargo.lock", "./"]
ADD rs ./rs

RUN apt-get update && apt-get install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  curl
RUN cargo install --path ./rs/voidmerge --bin vm

FROM debian:trixie-slim

RUN apt-get update && \
  apt-get install -y curl && \
  rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/local/cargo/bin/vm /usr/local/bin/

CMD ["vm"]
