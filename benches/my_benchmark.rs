use criterion::{criterion_group, criterion_main, Criterion};
use criterion::async_executor::FuturesExecutor;
use rod::{Node, Config};
use rod::adapters::{SledStorage, MemoryStorage, WsServer, OutgoingWebsocketManager};
use tokio::runtime::Runtime;
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicUsize, Ordering};
use rod::actor::Addr;
use rod::message::Message;

fn criterion_benchmark(c: &mut Criterion) {
    let rt  = Runtime::new().unwrap();
    c.bench_function("memory_storage get-put", |b| {
        rt.block_on(async {
            let mut db = Node::new_with_config(Config::default(), vec![Box::new(MemoryStorage::new())], vec![]);
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
            let config = Config::default();
            let sled = SledStorage::new_with_config(config.clone(), sled::Config::default().path(path), None);
            let mut db = Node::new_with_config(config.clone(), vec![Box::new(sled)], vec![]);
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
        rt.block_on(async {
            //sleep(Duration::from_millis(100)).await;
            let ws_server = Box::new(WsServer::new(Config::default()));
            let ws_client = Box::new(OutgoingWebsocketManager::new(Config::default(), vec!["http://localhost:4944/ws".to_string()]));
            let mut peer1 = Node::new_with_config(Config::default(), vec![Box::new(MemoryStorage::new())], vec![ws_server]);
            //sleep(Duration::from_millis(1000)).await; // let the server start
            let mut peer2 = Node::new_with_config(Config::default(), vec![Box::new(MemoryStorage::new())], vec![ws_client]);
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
    });
    group.finish();

    c.bench_function("parse and verify public space put json", |b| {
        let addr = Addr::noop();
        b.iter(|| {
            Message::try_from(r##"
            [
              {
                "put": {
                  "something": {
                    "_": {
                      "#": "something",
                      ">": {
                        "else": 1653465227430
                      }
                    },
                    "else": "{\"sig\":\"aSEA{\\\"m\\\":{\\\"text\\\":\\\"test post\\\",\\\"time\\\":\\\"2022-05-25T07:53:47.424Z\\\",\\\"type\\\":\\\"post\\\",\\\"author\\\":{\\\"keyID\\\":\\\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\\\"}},\\\"s\\\":\\\"WttDQegXyXILtB1nhNq7Jn69MZ0JD/b1LQrIybQ9UuHn86KvKXg9Lg7+ESmeqSQNaQy7KYvfBEEKbd/ClagQOQ==\\\"}\",\"pubKey\":\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\"}"
                  }
                },
                "#": "yvd2vk4338i"
              }
            ]
            "##, addr.clone(), true).unwrap();
        })
    });

    c.bench_function("parse and verify content addressed put json", |b| {
        let addr = Addr::noop();
        b.iter(|| {
            Message::try_from(r##"
            [
              {
                "put": {
                  "#": {
                    "_": {
                      "#": "#",
                      ">": {
                        "rkHfUdMssQ8Ln9LtiuPTb/ntNxR6HZiVdVsn9DdnKZs=": 1653465227430
                      }
                    },
                    "rkHfUdMssQ8Ln9LtiuPTb/ntNxR6HZiVdVsn9DdnKZs=": "{\"sig\":\"aSEA{\\\"m\\\":{\\\"text\\\":\\\"test post\\\",\\\"time\\\":\\\"2022-05-25T07:53:47.424Z\\\",\\\"type\\\":\\\"post\\\",\\\"author\\\":{\\\"keyID\\\":\\\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\\\"}},\\\"s\\\":\\\"WttDQegXyXILtB1nhNq7Jn69MZ0JD/b1LQrIybQ9UuHn86KvKXg9Lg7+ESmeqSQNaQy7KYvfBEEKbd/ClagQOQ==\\\"}\",\"pubKey\":\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\"}"
                  }
                },
                "#": "yvd2vk4338i"
              }
            ]
            "##, addr.clone(), false).unwrap();
        })
    });

    c.bench_function("parse and verify signed put json", |b| {
        let addr = Addr::noop();
        b.iter(|| {
            Message::try_from(r##"
            {
              "put": {
                "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8": {
                  "_": {
                    "#": "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8",
                    ">": {
                      "profile": 1653463165115
                    }
                  },
                  "profile": "{\":\":{\"#\":\"~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile\"},\"~\":\"JW+tFHHVBaY+zm/uzUoGVlogvXXQIA3vFNT0f0uX6tnnPGrRevDWzEmnVYy+ChxS6AJi5THiPyOc2HorIIM5wg==\"}"
                },
                "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile": {
                  "_": {
                    ">": {
                      "name": 1653463165115
                    },
                    "#": "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile"
                  },
                  "name": "{\":\":\"Arja Koriseva\",\"~\":\"KCq2D/T0mMenizxiVMso8FO5JIv9ZJLA0Q67DFa9qssPSKCmmieC1Nl5+nRpOX29C6A2/kLaJgphN/X7kUQjww==\"}"
                }
              },
              "#": "issWkzotF"
            }
            "##, addr.clone(), false).unwrap();
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);