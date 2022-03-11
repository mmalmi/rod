# Gun-rs

Pure Rust implementation of [Gun.js](https://github.com/amark/gun). For a wasm version, check out [gun-rs-wasm](https://github.com/mmalmi/gun-rs-wasm).

Deployed at https://gun-rs.iris.to (serves [iris-messenger](https://github.com/irislib/iris-messenger) at the root)

Live stats: https://gun-rs.iris.to/stats

## Why?
- Rust can be compiled into high-performing native binaries on many platforms, including embedded systems.
- Maintaining and contributing to the codebase is easier than in [Gun.js](https://github.com/amark/gun). Gun.js doesn't have compilation or minification steps, and the code is kind of manually minified 😄

## Use
Install [Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) first.

### Gun relay
```
cargo install gundb
gundb start
```

### Gun library
```rust
use gundb::{Node, NodeConfig};
use gundb::types::GunValue;
let mut db = Node::new_with_config(NodeConfig {
    outgoing_websocket_peers: vec!["wss://some-server-to-sync.with/gun".to_string()],
    ..NodeConfig::default()
});
let mut sub = db.get("greeting").on();
db.get("greeting").put("Hello World!".into());
if let GunValue::Text(str) = sub.recv().await.unwrap() {
    assert_eq!(&str, "Hello World!");
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
- [x] Advanced deduplication of sync messages
- [x] Publish & subscribe
- [ ] Disk storage
- [ ] SEA (verification of signed data)

### Issues
- When multiple adapters are enabled, it sometimes gets stuck
- User-space nodes (js: `gun.user().get('something')`) are relayed, but not properly retrieved and sent from memory (wrong format somehow)
- Multicast doesn't relay large messages like Iris posts

## Develop
```
cargo install cargo-watch
RUST_LOG=debug cargo watch -x 'run -- start'
```

## Run on Heroku
```
heroku create --buildpack emk/rust
git push heroku master
```

or:

[![Deploy](assets/herokubutton.svg)](https://heroku.com/deploy?template=https://github.com/mmalmi/gun-rs)
