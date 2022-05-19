use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gundb::{Node, Config};
use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration};

fn criterion_benchmark(c: &mut Criterion) {
    let rt  = Runtime::new().unwrap();
    c.bench_function("put-get", |b| b.iter(|| {
        rt.block_on(async {
            let mut db = Node::new_with_config(Config {
                sled_storage: false,
                memory_storage: true,
                websocket_server: false,
                ..Config::default()
            });
            for i in 0..100 {
                let mut node = db.get("a").get(&i.to_string());
                let mut sub = node.on();
                node.put("hello".into());
                sub.recv().await;
            }
            db.stop();
        });
    }));

    c.bench_function("put-get-sled", |b| b.iter(|| {
        let path = std::path::Path::new("./benchmark_db");
        std::fs::remove_dir_all(path);
        rt.block_on(async {
            let mut db = Node::new_with_config(Config {
                sled_storage: true,
                sled_config: sled::Config::default().path(path),
                memory_storage: false,
                websocket_server: false,
                ..Config::default()
            });
            for i in 0..100 {
                let mut node = db.get("a").get(&i.to_string());
                let mut sub = node.on();
                node.put("hello".into());
                sub.recv().await;
            }
            db.stop();
            sleep(Duration::from_millis(50)).await; // TODO: remove setup / teardown from measurement
            // https://bheisler.github.io/criterion.rs/book/user_guide/timing_loops.html
            std::fs::remove_dir_all(path);
        });
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);