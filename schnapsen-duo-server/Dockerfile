FROM rust:latest

WORKDIR /usr/schnapsen-duo-server
COPY Cargo.toml ./
COPY src/main.rs ./src/

RUN cargo build --release

COPY . .

RUN cargo build --release