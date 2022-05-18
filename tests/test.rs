#[cfg(test)]
mod tests {
    use gundb::{Node, Config, GunValue};
    use tokio::time::{sleep, Duration};
    use std::sync::Once;
    use log::{debug};

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    // TODO proper test
    // TODO test .map()
    // TODO benchmark
    #[tokio::test]
    async fn it_doesnt_error() {
        setup();
        let mut gun = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            ..Config::default()
        });
        let _ = gun.get("Meneldor"); // Pick Tolkien names from https://www.behindthename.com/namesakes/list/tolkien/alpha
    }

    #[tokio::test]
    async fn first_get_then_put() {
        setup();
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
        setup();
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
        setup();
        let mut node1 = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            websocket_server: true,
            multicast: false,
            stats: false,
            ..Config::default()
        });
        let mut node2 = Node::new_with_config(Config {
            memory_storage: true,
            sled_storage: false,
            websocket_server: false,
            multicast: false,
            stats: false,
            outgoing_websocket_peers: vec!["ws://localhost:4944/gun".to_string()],
            ..Config::default()
        });
        sleep(Duration::from_millis(2000)).await;
        let mut sub1 = node1.get("node2").get("name").on();
        let mut sub2 = node2.get("node1").get("name").on();
        node1.get("node1").get("name").put("Amandil".into());
        node2.get("node2").get("name").put("Beregond".into());
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
        node1.stop();
        node2.stop();
    }

    /*
    #[tokio::test]
    async fn sync_over_multicast() {
        let mut node1 = Node::new_with_config(Config {
            websocket_server: false,
            multicast: true,
            stats: false,
            ..Config::default()
        });
        let mut node2 = Node::new_with_config(Config {
            websocket_server: false,
            multicast: true,
            stats: false,
            ..Config::default()
        });
        async fn tst(mut node1: Node, mut node2: Node) {
            sleep(Duration::from_millis(1000)).await;
            node1.get("node1a").put("Gorlim".into());
            node2.get("node2a").put("Smaug".into());
            let mut sub1 = node1.get("node2a").on();
            let mut sub2 = node2.get("node1a").on();
            match sub1.recv().await.unwrap() {
                GunValue::Text(str) => {
                    assert_eq!(&str, "Smaug");
                },
                _ => panic!("Expected GunValue::Text")
            }
            match sub2.recv().await.unwrap() {
                GunValue::Text(str) => {
                    assert_eq!(&str, "Gorlim");
                },
                _ => panic!("Expected GunValue::Text")
            }
            node1.stop();
            node2.stop();
        }
        let node1_clone = node1.clone();
        let node2_clone = node2.clone();
        tokio::join!(node1.start(), node2.start(), tst(node1_clone, node2_clone));
    }*/

    /*

    #[test]
    fn save_and_retrieve_user_space_data() {
        setup();
        let mut node = Node::new();
    }

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
