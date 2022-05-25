use criterion::{criterion_group, criterion_main, Criterion};
use criterion::async_executor::FuturesExecutor;
use rod::{Node, Config};
use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicUsize, Ordering};

fn criterion_benchmark(c: &mut Criterion) {
    let rt  = Runtime::new().unwrap();
    c.bench_function("memory_storage get-put", |b| {
        rt.block_on(async {
            let mut db = Node::new_with_config(Config {
                sled_storage: false,
                memory_storage: true,
                websocket_server: false,
                ..Config::default()
            });
            let counter: AtomicUsize = AtomicUsize::new(0);
            b.to_async(FuturesExecutor).iter(|| {
                let mut db = db.clone();
                let key = counter.fetch_add(1, Ordering::Relaxed).to_string();
                async move {
                    let mut node = db.get("a").get(&key);
                    node.put("hello".into());
                    let mut sub = node.on();
                    sub.recv().await.ok();
                }
            });
            db.stop();
        });
    });

    let mut group = c.benchmark_group("fewer samples");
    group.sample_size(10);

    group.bench_function("sled_storage get-put", |b| {
        let path = std::path::Path::new("./benchmark_db");
        std::fs::remove_dir_all(path).ok();
        rt.block_on(async {
            sleep(Duration::from_millis(100)).await;
            let mut db = Node::new_with_config(Config {
                sled_storage: true,
                sled_config: sled::Config::default().path(path),
                memory_storage: false,
                websocket_server: false,
                ..Config::default()
            });
            let counter: AtomicUsize = AtomicUsize::new(0);
            b.to_async(FuturesExecutor).iter(|| {
                let mut db = db.clone();
                let key = counter.fetch_add(1, Ordering::Relaxed).to_string();
                async move {
                    let mut node = db.get("a").get(&key);
                    node.put("hello".into());
                    let mut sub = node.on();
                    sub.recv().await.ok();
                }
            });
            db.stop(); // should this be awaitable?
            sleep(Duration::from_millis(100)).await;
        });
        // https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html
        std::fs::remove_dir_all(path).ok();
    });

    group.bench_function("websocket get-put", |b| {
        let path1 = std::path::Path::new("./benchmark1_db");
        let path2 = std::path::Path::new("./benchmark2_db");
        std::fs::remove_dir_all(path1).ok();
        std::fs::remove_dir_all(path2).ok();
        rt.block_on(async {
            //sleep(Duration::from_millis(100)).await;
            let mut peer1 = Node::new_with_config(Config {
                sled_storage: false,
                sled_config: sled::Config::default().path(path1),
                memory_storage: true,
                websocket_server: true,
                ..Config::default()
            });
            //sleep(Duration::from_millis(1000)).await; // let the server start
            let mut peer2 = Node::new_with_config(Config {
                sled_storage: false,
                sled_config: sled::Config::default().path(path2),
                memory_storage: true,
                websocket_server: false,
                outgoing_websocket_peers: vec!["ws://localhost:4944/ws".to_string()],
                ..Config::default()
            });
            //sleep(Duration::from_millis(1000)).await; // let the ws connect
            let counter: AtomicUsize = AtomicUsize::new(0);
            b.to_async(FuturesExecutor).iter(|| {
                let mut peer1 = peer1.clone();
                let mut peer2 = peer2.clone();
                let key = counter.fetch_add(1, Ordering::Relaxed).to_string();
                async move {
                    peer1.get("a").get(&key).put("hello".into());
                    let mut sub = peer2.get("a").get(&key).on();
                    //sub.recv().await; // TODO enable
                }
            });
            peer1.stop(); // should this be awaitable?
            peer2.stop(); // should this be awaitable?
            sleep(Duration::from_millis(100)).await;
        });
        // https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html
        std::fs::remove_dir_all(path1).ok();
        std::fs::remove_dir_all(path2).ok();
    });
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);