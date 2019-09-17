FROM rust:1-slim-buster as builder
RUN apt-get update && apt-get install -yq \
    cmake \
    libssl-dev \
    ninja-build \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /work
ADD https://github.com/nanomsg/nng/archive/v1.1.1.tar.gz ./nng.tar.gz
RUN tar xvzf nng.tar.gz && \
    mkdir build
WORKDIR /work/build
RUN cmake -G Ninja ../nng-1.1.1 && \
    ninja && \
    ninja install
COPY Cargo.toml Cargo.lock /work/
COPY crates/ /work/crates/
RUN cargo fetch
RUN cargo build --all --release

FROM debian:buster-slim
RUN apt-get update && apt-get install -yq ca-certificates libssl1.1
COPY cert/ /opt/cert/
COPY --from=builder /work/target/release/rendezvous-* /usr/local/bin/

LABEL org.label-schema.schema-version="1.0" \
    org.label-schema.name="rendezvous" \
    org.label-schema.vcs-url="https://github.com/langdev/rendezvous"
