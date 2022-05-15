# Gun-rs

Pure Rust implementation of [Gun.js](https://github.com/amark/gun). For a wasm version, check out [gun-rs-wasm](https://github.com/mmalmi/gun-rs-wasm).

[Iris-messenger](https://github.com/irislib/iris-messenger) uses the gun-rs node at wss://gun-rs.iris.to/gun

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
15/5/2022:

- [x] Gun basic API
- [x] CLI for running the server
- [x] Incoming websockets
- [x] Outgoing websockets (env PEERS=wss://some-server-url.herokuapp.com/gun)
- [x] Multicast (currently size limited to 65KB — large photos in messages will not sync over it)
- [x] In-memory storage
- [x] TLS support (env CERT_PATH and KEY_PATH)
- [x] Advanced deduplication of network messages
- [x] Publish & subscribe (network messages only relayed to relevant peers)
- [x] Disk storage ([sled.rs](https://sled.rs))
- [x] Hash verification for content-addressed data (`db.get('#').get(data_hash).put(data)`)
- [ ] Signature verification of user data (`db.get('~' + pubkey).get('profile') ...`)
- [ ] Encryption & decryption (usually done on the client side in js, like [iris](https://github.com/iris-lib/iris-messenger) private messaging)

### Issues
- Multicast doesn't relay large messages like Iris posts
- Rust API .on() and .map() not working correctly as of 15/5/2022

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
