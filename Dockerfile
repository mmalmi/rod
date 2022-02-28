FROM rust:1.59

WORKDIR /usr/src/gun-rs
COPY . .

RUN cargo install --path .

CMD ["gundb start"]