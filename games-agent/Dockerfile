FROM rust:latest

WORKDIR /usr/

RUN USER=root cargo new --bin games-agent
WORKDIR /usr/games-agent

COPY Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/games_agent*
RUN cargo install --path .