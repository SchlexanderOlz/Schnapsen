FROM rust:latest

WORKDIR /usr/schnapsen-duo-server
COPY Cargo.toml Cargo.lock ./

RUN cargo build --release

COPY . .

RUN cargo build --release