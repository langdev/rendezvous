FROM langdev/rendezvous:vendor as builder
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
