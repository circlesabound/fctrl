# fctrl - All-in-one Factorio server management

`fctrl` is a tool to manage your Factorio multiplayer server.

***THIS IS A WORK IN PROGRESS***

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
- NodeJS, NPM, and Angular CLI installed globally

Installation:

1. Clone the repository, or download a .zip of the source code and extract.
2. Run `npm install` to restore Node-based build tools.
3. In the `web` directory, run `npm install` to restore dependencies for the Angular build, then run `ng build --prod` to build the web interface.
4. Configure the values in `.env`, then apply them by running `source .env`.
5. To run the agent application, run `cargo run --release --bin agent`.
6. To run the management web server, run `cargo run --release --bin mgmt-server`.

## Architecture

`fctrl` consists of two runtime applications: `agent` and `mgmt-server`.

### `agent`

`agent` is responsible for managing the actual Factorio executable and its dependencies, including installation and launching of the application itself, saves, config files, and mods. It interfaces with a running Factorio process by capturing the process stdout/stderr as well as through an RCON communication channel.

`agent` is controlled via a WebSocket API, which is how the above functionality is exposed. Logs and output from the Factorio process are also streamed via WebSocket to any connected clients. In your typical setup, the only WebSocket client would be the `mgmt-server`.

`agent` is built as a Rust application, and has no additional runtime dependencies.

### `mgmt-server`

`mgmt-server` serves as the interface between the user and the `agent` application. It does this by providing a web-based interface with which users can control the operation of the `agent`. The user-facing components of `mgmt-server` can be broken into two components:

- A "friendly" web frontend, created using Angular.
- A backend application serving the static frontend and a mixed REST / WebSocket API, written in Rust.

The mixed REST / WebSocket API provided by the backend portion of `mgmt-server` is a user-friendly encapsulation of the functionality exposed by the `agent`'s WebSocket API. TODO example

The backend application of `mgmt-server` also acts as a log ingestion service for the `agent` - logs streamed from the `agent` are stored in a [RocksDB](https://rocksdb.org/) database for future perusal.
