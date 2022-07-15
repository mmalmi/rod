####################################################################################################
## Builder
####################################################################################################
FROM rust:latest AS builder

WORKDIR /app
COPY Cargo.toml .
COPY Cargo.lock .
COPY benches benches
COPY assets assets
COPY src src

RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release --bin rod && \
    mv /app/target/release/rod .

####################################################################################################
## Final image
####################################################################################################
FROM gcr.io/distroless/cc
COPY assets /assets
COPY --from=builder /app/rod /
EXPOSE 4944 4945
ENTRYPOINT ["./rod"]