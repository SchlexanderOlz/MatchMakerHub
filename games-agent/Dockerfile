FROM rust:latest

WORKDIR /usr/

ARG AMQP_URL
ARG DEBUG

ENV AMQP_URL=${AMQP_URL}
ENV DEBUG=${DEBUG}


RUN USER=root cargo new --bin games-agent
WORKDIR /usr/games-agent

COPY Cargo.toml ./
COPY build.rs ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/games_agent*
RUN cargo install --path .