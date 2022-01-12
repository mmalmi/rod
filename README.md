# Gun-rs

Pure Rust implementation of [Gun](https://github.com/amark/gun). For a wasm version, check out [gun-rs-wasm](https://github.com/mmalmi/gun-rs-wasm).

Deployed at https://gun-us.herokuapp.com (serves [iris-messenger](https://github.com/irislib/iris-messenger) at the root)

Live stats: https://gun-us.herokuapp.com/stats

## Status
3/12/2021:

- [x] Gun basic API
- [x] Incoming websockets
- [x] Outgoing websockets (env PEERS=wss://some-server-url.herokuapp.com/gun)
- [x] Multicast (Iris messages seem not to propagate — size limit?)
- [x] In-memory storage
- [ ] TLS support (env CERT_PATH and KEY_PATH)
- [ ] Disk storage
- [ ] SEA
- [ ] Advanced deduplication

## Develop
[Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) is required.

```
cargo install cargo-watch
RUST_LOG=debug cargo watch -x 'run -- serve'
```

## Run on Heroku
```
heroku create --buildpack emk/rust
git push heroku master
```

or:

[![Deploy](assets/herokubutton.svg)](https://heroku.com/deploy?template=https://github.com/mmalmi/gun-rs)
