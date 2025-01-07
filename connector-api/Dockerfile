FROM rust:latest

ARG HOST_ADDR
ARG DEBUG

ENV HOST_ADDR=${HOST_ADDR}
ENV DEBUG=${DEBUG}

WORKDIR /usr/

RUN USER=root cargo new --bin connector-api
WORKDIR /usr/connector-api

COPY Cargo.toml ./
COPY build.rs ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/connector_api*
RUN cargo install --path .