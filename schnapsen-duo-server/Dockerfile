FROM rust:latest AS builder

ARG AMQP_URL
ARG NEXT_ROUND_DELAY

WORKDIR /usr/
RUN USER=root cargo new --bin schnapsen-duo-server
WORKDIR /usr/schnapsen-duo-server

COPY Cargo.toml ./

RUN cargo build
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/debug/deps/schnapsen_duo_server*
RUN cargo build

FROM rust:latest
COPY --from=builder /usr/schnapsen-duo-server/target/debug/schnapsen-duo-server /usr/local/bin/schnapsen-duo-server
CMD ["schnapsen-duo-server"]
