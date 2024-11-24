FROM rust:latest

WORKDIR /usr/

RUN USER=root cargo new --bin schnapsen-duo-server 
WORKDIR /usr/schnapsen-duo-server

COPY Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/schnapsen_duo_server*
RUN cargo install --path .