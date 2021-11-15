# Gun-rs

Pure Rust implementation of Gun. As of 15/11/2021 has in-memory and websocket server functionality. For a wasm version, check out [rusty-gun](https://github.com/mmalmi/rusty-gun).

Deployed at https://gun-rs.herokuapp.com (serves [iris-messenger](https://github.com/irislib/iris-messenger) at the root)

Live stats: https://gun-rs.herokuapp.com/stats

## Develop
[Rust](https://doc.rust-lang.org/book/ch01-01-installation.html) is required.

```
cargo install cargo-watch
cargo watch -x 'run -- serve'
```

## Run on Heroku
```
heroku create --buildpack emk/rust
git push heroku master
```

or:

[![Deploy](assets/herokubutton.svg)](https://heroku.com/deploy?template=https://github.com/mmalmi/rod)
