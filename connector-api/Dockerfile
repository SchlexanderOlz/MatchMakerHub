FROM rust:latest

ARG HOST_ADDR

WORKDIR /usr/

RUN USER=root cargo new --bin connector-api
WORKDIR /usr/connector-api

COPY Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/connector_api*
RUN cargo install --path .