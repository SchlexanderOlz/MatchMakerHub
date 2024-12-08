FROM rust:latest

WORKDIR /usr/

RUN USER=root cargo new --bin matchmaking-state-api
WORKDIR /usr/matchmaking-state-api

COPY Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/matchmaking_state_api*
RUN cargo install --path .