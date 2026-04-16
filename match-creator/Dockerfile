FROM rust:latest AS builder

WORKDIR /usr/
RUN USER=root cargo new --bin match-creator
WORKDIR /usr/match-creator

COPY Cargo.toml ./

RUN cargo build
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/debug/deps/match_creator*
RUN cargo build

FROM rust:latest
COPY --from=builder /usr/match-creator/target/debug/match-creator /usr/local/bin/match-creator
CMD ["match-creator"]
