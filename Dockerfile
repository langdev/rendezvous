FROM spoqa/rust:nightly-minideb as builder
RUN apt-get update && apt-get install -yq libssl-dev
WORKDIR /work
RUN apt-get install -yq pkg-config
COPY Cargo.toml Cargo.lock /work/
RUN cargo fetch
COPY src/ /work/src/
RUN cargo build

FROM bitnami/minideb
RUN apt-get update && apt-get install -yq libssl-dev ca-certificates
COPY --from=builder /work/target/debug/rendezvous /usr/local/bin/
CMD ["rendezvous"]
