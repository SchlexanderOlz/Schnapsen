FROM rust:latest

WORKDIR /usr/schnapsen-duo-server
COPY Cargo.toml ./
COPY main.rs ./

RUN cargo build --release

COPY . .

RUN cargo build --release