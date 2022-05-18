#[cfg(test)]
mod tests {
    use gundb::{Node, Config, GunValue};
    use tokio::time::{sleep, Duration};
    use std::sync::Once;
    use log::{debug};

    static INIT: Once = Once::new();

    fn enable_logger() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    // TODO proper test
    // TODO test .map()
    // TODO benchmark
    #[tokio::test]
    async fn it_doesnt_error() {
        let mut gun = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            ..Config::default()
        });
        let _ = gun.get("Meneldor"); // Pick Tolkien names from https://www.behindthename.com/namesakes/list/tolkien/alpha
    }

    #[tokio::test]
    async fn first_get_then_put() {
        let mut gun = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            ..Config::default()
        });
        let mut node = gun.get("Anborn");
        let mut sub = node.on();
        node.put("Ancalagon".into());
        if let GunValue::Text(str) = sub.recv().await.unwrap() {
            assert_eq!(&str, "Ancalagon");
        }
    }

    #[tokio::test]
    async fn first_put_then_get() {
        let mut gun = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            ..Config::default()
        });
        let mut node = gun.get("Finglas");
        node.put("Fingolfin".into());
        let mut sub = node.on();
        if let GunValue::Text(str) = sub.recv().await.unwrap() {
            assert_eq!(&str, "Fingolfin");
        }
    }

    #[tokio::test]
    async fn connect_and_sync_over_websocket() {
        let mut peer1 = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            websocket_server: true,
            multicast: false,
            stats: false,
            ..Config::default()
        });
        let mut peer2 = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            websocket_server: false,
            multicast: false,
            stats: false,
            outgoing_websocket_peers: vec!["ws://localhost:4944/gun".to_string()],
            ..Config::default()
        });
        sleep(Duration::from_millis(2000)).await;
        let mut sub1 = peer1.get("beta").get("name").on();
        let mut sub2 = peer2.get("alpha").get("name").on();
        peer1.get("alpha").get("name").put("Amandil".into());
        peer2.get("beta").get("name").put("Beregond".into());
        match sub1.recv().await.unwrap() {
            GunValue::Text(str) => {
                assert_eq!(&str, "Beregond");
            },
            _ => panic!("Expected GunValue::Text")
        }
        match sub2.recv().await.unwrap() {
            GunValue::Text(str) => {
                assert_eq!(&str, "Amandil");
            },
            _ => panic!("Expected GunValue::Text")
        }
        peer1.stop();
        peer2.stop();
    }

    #[tokio::test]
    async fn sync_over_multicast() {
        enable_logger();
        let mut peer1 = Node::new_with_config(Config {
            websocket_server: false,
            multicast: true,
            stats: false,
            sled_storage: false,
            ..Config::default()
        });
        let mut peer2 = Node::new_with_config(Config {
            websocket_server: false,
            multicast: true,
            stats: false,
            sled_storage: false,
            ..Config::default()
        });
        sleep(Duration::from_millis(1000)).await;
        peer1.get("gamma").put("Gorlim".into());
        peer2.get("sigma").put("Smaug".into());
        let mut sub1 = peer1.get("sigma").on();
        let mut sub2 = peer2.get("gamma").on();
        match sub1.recv().await.unwrap() {
            GunValue::Text(str) => {
                assert_eq!(&str, "Smaug");
            },
            _ => panic!("Expected GunValue::Text")
        };
        match sub2.recv().await.unwrap() {
            GunValue::Text(str) => {
                assert_eq!(&str, "Gorlim");
            },
            _ => panic!("Expected GunValue::Text")
        };
        peer1.stop();
        peer2.stop();
    }

    /*
    #[test] // use #[bench] when it's stable
    fn write_benchmark() { // to see the result with optimized binary, run: cargo test --release -- --nocapture
        setup();
        let start = Instant::now();
        let mut gun = Node::new();
        let n = 1000;
        for i in 0..n {
            gun.get(&format!("a{:?}", i)).get("Pelendur").put(format!("{:?}b", i).into());
        }
        let duration = start.elapsed();
        let per_second = (n as f64) / (duration.as_nanos() as f64) * 1000000000.0;
        println!("Wrote {} entries in {:?} ({} / second)", n, duration, per_second);
        // compare with gun.js: var i = 100000, j = i, s = +new Date; while(--i){ gun.get('a'+i).get('lol').put(i+'yo') } console.log(j / ((+new Date - s) / 1000), 'ops/sec');
    }
     */
}
