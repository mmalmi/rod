####################################################################################################
## Builder
####################################################################################################
FROM clux/muslrust:stable AS builder

COPY Cargo.toml .
COPY Cargo.lock .
COPY benches benches
COPY assets assets
COPY src src

RUN --mount=type=cache,target=/volume/target \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release --bin rod && \
    mv /volume/target/x86_64-unknown-linux-musl/release/rod .

####################################################################################################
## Final image
####################################################################################################
FROM gcr.io/distroless/static:nonroot
COPY --from=builder --chown=nonroot:nonroot /volume/rod /app/
EXPOSE 4944 4945
ENTRYPOINT ["/app/rod"]