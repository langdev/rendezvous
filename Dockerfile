FROM rust:1-buster as vendor
RUN apt-get update && apt-get install -yq \
    cmake \
    libssl-dev \
    ninja-build \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
ADD https://github.com/nanomsg/nng/archive/v1.1.1.tar.gz /work/nng.tar.gz
WORKDIR /work/build
RUN tar xvzf /work/nng.tar.gz -C /work/ && \
    cmake -G Ninja ../nng-1.1.1 && \
    ninja && \
    ninja install
WORKDIR /work
COPY Cargo.toml Cargo.lock /work/
COPY crates/ /work/crates/
COPY deploy/cargo.config /work/.cargo/config
RUN cargo vendor

FROM vendor as builder
COPY Cargo.toml Cargo.lock /work/
COPY crates/ /work/crates/
RUN cargo build --all --release

FROM debian:buster-slim
RUN apt-get update && apt-get install -yq ca-certificates libssl1.1
COPY cert/ /opt/cert/
COPY --from=builder /work/target/release/rendezvous-* /usr/local/bin/

LABEL org.label-schema.schema-version="1.0" \
    org.label-schema.name="rendezvous" \
    org.label-schema.vcs-url="https://github.com/langdev/rendezvous"
