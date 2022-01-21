# Gun-rs

Pure Rust implementation of [Gun](https://github.com/amark/gun). For a wasm version, check out [gun-rs-wasm](https://github.com/mmalmi/gun-rs-wasm).

Deployed at https://gun-us.herokuapp.com (serves [iris-messenger](https://github.com/irislib/iris-messenger) at the root)

Live stats: https://gun-us.herokuapp.com/stats

## Use
Install [Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) first.

### Gun server
```
cargo install gundb
gundb serve
```

### Gun library
```rust
use gundb::{Node, NodeConfig};
use gundb::types::GunValue;
let mut db = Node::new_with_config(NodeConfig {
    outgoing_websocket_peers: vec!["wss://some-server-to-sync.with/gun".to_string()],
    ..NodeConfig::default()
});
let sub = db.get("greeting").on();
db.get("greeting").put("Hello World!".into());
if let Ok(value) = sub.recv().await {
    if let GunValue::Text(str) = value {
        assert_eq!(&str, "Hello World!");
    }
}
```

## Status
3/12/2021:

- [x] Gun basic API
- [x] CLI for running the server
- [x] Incoming websockets
- [x] Outgoing websockets (env PEERS=wss://some-server-url.herokuapp.com/gun)
- [x] Multicast (Iris messages seem not to propagate — size limit?)
- [x] In-memory storage
- [x] TLS support (env CERT_PATH and KEY_PATH)
- [ ] Disk storage
- [ ] SEA
- [ ] Advanced deduplication

## Develop
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
