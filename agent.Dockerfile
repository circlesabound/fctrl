FROM ekidd/rust-musl-builder:latest AS prepare
RUN cargo install cargo-chef --version 0.1.19
# ekidd/rust-musl-builder sets WORKDIR to /home/rust/src, with the correct perms
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM ekidd/rust-musl-builder:latest AS cache
RUN cargo install cargo-chef --version 0.1.19
COPY --from=prepare /home/rust/src/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM ekidd/rust-musl-builder:latest AS builder
COPY --from=cache /home/rust/src/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
COPY . .
RUN cargo build --release --bin agent

FROM frolvlad/alpine-glibc:latest AS runtime
WORKDIR /app
COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release /usr/local/bin
ENTRYPOINT [ "/usr/local/bin/agent" ]
