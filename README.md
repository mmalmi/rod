# Rod

Rust Object Database.

The decentralized social networking application [Iris-messenger](https://github.com/irislib/iris-messenger) syncs over Rod peers by default.

## Use

Install [Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) first.

### Install & run

```
cargo install rod
rod start
```

### Library

```rust
use rod::{Node, Config, Value};
let mut db = Node::new_with_config(Config {
    outgoing_websocket_peers: vec!["wss://some-server-to-sync.with/ws".to_string()],
    ..Config::default()
});
let mut sub = db.get("greeting").on();
db.get("greeting").put("Hello World!".into());
if let Value::Text(str) = sub.recv().await.unwrap() {
    assert_eq!(&str, "Hello World!");
}
```

## Status

15/5/2022:

- [x] Basic 
- [x] CLI for running the server
- [x] Incoming websockets
- [x] Outgoing websockets (env PEERS=wss://some-server-url.herokuapp.com/ws)
- [x] Multicast (currently size limited to 65KB — large photos in messages will not sync over it)
- [x] In-memory storage
- [x] TLS support (env CERT_PATH and KEY_PATH)
- [x] Advanced deduplication of network messages
- [x] Publish & subscribe (network messages only relayed to relevant peers)
- [x] Disk storage ([sled.rs](https://sled.rs))
- [x] Hash verification for content-addressed data (`db.get('#').get(data_hash).put(data)`)
- [x] Signature verification of user data (`db.get('~' + pubkey).get('profile') ...`)
- [ ] Encryption & decryption (usually not needed on the server, but used on the client side in js, like [iris](https://github.com/iris-lib/iris-messenger) private messaging)

### Issues

- Multicast doesn't relay large messages like Iris posts with photos

## Develop

```
cargo install cargo-watch
RUST_LOG=debug cargo watch -x 'run -- start'
```

```
cargo test
```

```
cargo bench
```

## Run on Heroku

```
heroku create --buildpack emk/rust
git push heroku master
```

or:

[![Deploy](assets/herokubutton.svg)](https://heroku.com/deploy?template=https://github.com/mmalmi/rod)
