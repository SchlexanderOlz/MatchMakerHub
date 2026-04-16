FROM rust:latest AS builder

ARG HOST_ADDR
ARG DEBUG

ENV HOST_ADDR=${HOST_ADDR}
ENV DEBUG=${DEBUG}

WORKDIR /usr/
RUN USER=root cargo new --bin connector-api
WORKDIR /usr/connector-api

COPY Cargo.toml ./
COPY build.rs ./

RUN cargo build
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/debug/deps/connector_api*
RUN cargo build

FROM rust:latest
COPY --from=builder /usr/connector-api/target/debug/connector-api /usr/local/bin/connector-api
CMD ["connector-api"]
