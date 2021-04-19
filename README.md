# fctrl - All-in-one Factorio server management

`fctrl` is a tool to manage your Factorio multiplayer server.

## Features

- Fully managed installation and upgrade process of the Factorio headless server software
- Server configuration, including admin list, multiplayer white/ban lists, and the advanced settings in `server-settings.json`
- Mod management, including changing mod settings
- RCON terminal
- Server log capture and ingestion
- A convenient Web UI for all the above!

## Usage (Docker)

`fctrl` is designed to be run on Docker, and a `docker-compose.yml` file is provided for ease of use.

Requirements:

- Docker
- Docker Compose

Installation:

1. From the repository root, download the `docker-compose.yml` file and the `.env` file to the same location.
2. If desired, configure the values in `.env`.
3. Run `docker-compose up -d`.

## Usage (Linux)

Alternatively, you can run `fctrl` without Docker, on Linux (not tested on any other OS). This is not recommended due to the many depedencies required, and binaries are not provided so you will have to build from source.

Requirements:

- Cargo and a nightly build of Rust
- Clang/LLVM
- Java 11
- NodeJS and NPM

Installation:

1. Clone the repository, or download a .zip of the source code and extract.
2. Run `npm install` to restore Node-based build tools.
3. Configure the values in `.env`, then apply them by running `source .env`.
4. To run the agent application, run `cargo run --release --bin agent`.
5. To run the management web server, run `cargo run --release --bin mgmt-server`.

// TODO angular build?

## Architecture

TODO
