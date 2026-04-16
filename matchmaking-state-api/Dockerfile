FROM rust:latest AS builder

WORKDIR /usr/
RUN USER=root cargo new --bin matchmaking-state-api
WORKDIR /usr/matchmaking-state-api

COPY Cargo.toml ./

RUN cargo build
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/debug/deps/matchmaking_state_api*
RUN cargo build

FROM rust:latest
COPY --from=builder /usr/matchmaking-state-api/target/debug/matchmaking-state-api /usr/local/bin/matchmaking-state-api
CMD ["matchmaking-state-api"]
