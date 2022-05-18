use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gundb::{Node, Config};
use tokio::runtime::Runtime;

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
        });
    }));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);