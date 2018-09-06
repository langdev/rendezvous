FROM rustlang/rust:nightly-slim as builder
RUN apt-get update && apt-get install -yq \
    libssl-dev \
    pkg-config \
    upx-ucl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /work
COPY Cargo.toml Cargo.lock /work/
RUN cargo fetch
COPY src/ /work/src/
RUN cargo build --release && upx -9 /work/target/release/rendezvous

FROM debian:stretch-slim
RUN apt-get update && apt-get install -yq ca-certificates libssl1.1
COPY cert/ /opt/cert/
COPY --from=builder /work/target/release/rendezvous /usr/local/bin/

LABEL org.label-schema.schema-version="1.0" \
    org.label-schema.name="rendezvous" \
    org.label-schema.vcs-url="https://github.com/langdev/rendezvous"

ENTRYPOINT ["rendezvous"]
