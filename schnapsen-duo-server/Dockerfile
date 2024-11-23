FROM rust:latest

WORKDIR /usr/schnapsen-duo-server
COPY Cargo.toml ./

RUN cargo build --release

COPY . .

RUN cargo build --release