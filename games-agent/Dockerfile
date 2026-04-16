FROM rust:latest AS builder

ARG AMQP_URL
ARG DEBUG

ENV AMQP_URL=${AMQP_URL}
ENV DEBUG=${DEBUG}

WORKDIR /usr/
RUN USER=root cargo new --bin games-agent
WORKDIR /usr/games-agent

COPY Cargo.toml ./
COPY build.rs ./

RUN cargo build
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/debug/deps/games_agent*
RUN cargo build

FROM rust:latest
COPY --from=builder /usr/games-agent/target/debug/games-agent /usr/local/bin/games-agent
CMD ["games-agent"]
