FROM rust:latest


WORKDIR /usr/

RUN USER=root cargo new --bin match-creator 
WORKDIR /usr/match-creator

COPY Cargo.toml ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/match_creator*
RUN cargo install --path .