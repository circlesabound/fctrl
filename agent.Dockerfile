FROM rustlang/rust:nightly AS prepare
WORKDIR /usr/src/app
RUN cargo install cargo-chef --version 0.1.19
COPY Cargo.toml .
COPY Cargo.lock .
COPY tests tests
COPY src src
RUN cargo chef prepare --recipe-path recipe.json

FROM rustlang/rust:nightly AS cache
WORKDIR /usr/src/app
RUN cargo install cargo-chef --version 0.1.19
RUN apt update
RUN apt install -y clang
COPY --from=prepare /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rustlang/rust:nightly AS builder
WORKDIR /usr/src/app
RUN curl -fsSL https://deb.nodesource.com/setup_15.x | bash \
    && apt update \
    && apt install -y clang nodejs openjdk-11-jre-headless
COPY openapitools.json .
COPY package-lock.json .
COPY package.json .
RUN npm install
COPY build.rs .
COPY --from=cache /usr/src/app/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
COPY Cargo.toml .
COPY Cargo.lock .
COPY openapi openapi
COPY tests tests
COPY src src
RUN cargo build --release --bin agent

FROM debian:buster-slim AS runtime
WORKDIR /app
RUN apt update
RUN apt install -y ca-certificates
COPY --from=builder /usr/src/app/target/release /usr/local/bin
ENTRYPOINT [ "/usr/local/bin/agent" ]
