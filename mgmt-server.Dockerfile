FROM rustlang/rust:nightly AS prepare
WORKDIR /usr/src/app
COPY rust-toolchain.toml .
RUN cargo install cargo-chef --version 0.1.66
COPY Cargo.toml .
COPY Cargo.lock .
COPY tests tests
COPY src src
RUN cargo chef prepare --recipe-path recipe.json

FROM rustlang/rust:nightly AS cache
WORKDIR /usr/src/app
RUN apt-get update \
    && apt-get install -y clang
COPY rust-toolchain.toml .
RUN cargo install cargo-chef --version 0.1.66
COPY --from=prepare /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

FROM rustlang/rust:nightly AS builder
WORKDIR /usr/src/app
RUN apt-get update \
    && apt-get install -y ca-certificates curl gnupg \
    && mkdir -p /etc/apt/keyrings \
    && curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg \
    && NODE_MAJOR=20 \
    && echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_$NODE_MAJOR.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list
RUN apt-get update \
    && apt-get install -y clang nodejs openjdk-17-jre-headless libgit2-1.5
COPY openapitools.json .
COPY package-lock.json .
COPY package.json .
RUN npm install
COPY rust-toolchain.toml .
COPY build.rs .
COPY --from=cache /usr/src/app/target target
COPY --from=cache $CARGO_HOME $CARGO_HOME
COPY Cargo.toml .
COPY Cargo.lock .
COPY openapi openapi
COPY tests tests
COPY src src
ARG GIT_COMMIT_HASH=-
ENV GIT_COMMIT_HASH=${GIT_COMMIT_HASH}
RUN cargo build --release --bin mgmt-server

FROM node:lts-alpine AS web-builder
WORKDIR /app/web
COPY web/package.json /app/web/package.json
COPY web/package-lock.json /app/web/package-lock.json
RUN npm install
COPY web /app/web
COPY openapi /app/openapi
RUN npm run build -- --configuration production

FROM debian:bookworm-slim AS runtime
WORKDIR /app
RUN apt-get update \
    && apt-get install -y ca-certificates
COPY --from=builder /usr/src/app/target/release/mgmt-server /usr/local/bin/mgmt-server
COPY --from=web-builder /app/web/dist/web /app/web/dist/web
ENTRYPOINT [ "/usr/local/bin/mgmt-server" ]
