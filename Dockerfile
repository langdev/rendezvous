FROM spoqa/rust:nightly as base
RUN apt-get update && apt-get install -yq libssl-dev

FROM base as builder
WORKDIR /work
RUN apt-get install -yq pkg-config
COPY Cargo.toml Cargo.lock /work/
RUN cargo fetch
COPY src/ /work/src/
RUN cargo build

FROM base
RUN apt-get update && apt-get install -yq libssl-dev
COPY --from=builder /work/target/debug/rendezvous /usr/local/bin/
CMD ["rendezvous"]
