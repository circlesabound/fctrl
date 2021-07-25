FROM rustlang/rust:nightly AS prepare
WORKDIR /usr/src/app
RUN cargo install cargo-chef --version 0.1.22
COPY Cargo.toml .
COPY Cargo.lock .
COPY tests tests
COPY src src
RUN cargo chef prepare --recipe-path recipe.json

FROM rustlang/rust:nightly AS cache
WORKDIR /usr/src/app
RUN cargo install cargo-chef --version 0.1.22
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
RUN cargo build --release --bin mgmt-server

FROM node:alpine AS web-builder
WORKDIR /app/web
COPY web/package.json /app/web/package.json
COPY web/package-lock.json /app/web/package-lock.json
RUN npm install
COPY web /app/web
RUN npm run ng -- build --prod

FROM debian:buster-slim AS runtime
WORKDIR /app
RUN apt update
RUN apt install -y ca-certificates
COPY --from=builder /usr/src/app/target/release /usr/local/bin
COPY --from=web-builder /app/web/dist/web /app/web/dist/web
ENTRYPOINT [ "/usr/local/bin/mgmt-server" ]
