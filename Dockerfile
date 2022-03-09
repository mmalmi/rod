FROM rust:latest

WORKDIR /usr/src/gun-rs
COPY . .

RUN cargo install --path .
