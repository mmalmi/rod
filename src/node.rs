use std::collections::BTreeMap;
use std::sync::{
    Arc,
    RwLock // TODO: could we use async RwLock? Would require some changes to the chaining api (.get()).
};
use std::time::SystemTime;
use crate::router::Router;
use crate::message::{Message, Put, Get};
use crate::types::*;
use crate::actor::{Addr, ActorContext};
use crate::utils::random_string;
use log::{debug, info};
use tokio::sync::broadcast;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

// TODO extract networking to struct Mesh
// TODO proper automatic tests
// Node { node: Arc<RwLock<NodeInner>> } instead of Arc<RwLock> for each member? compare performance
// TODO connections don't seem to be closed / timeouted properly when client has disconnected
// TODO should use async RwLock everywhere?

// TODO: separate configs for each adapter?
/// [Node] configuration object.
#[derive(Clone)]
pub struct Config {
    /// [tokio::sync::broadcast] channel size for outgoing network messages. Smaller value may slightly reduce memory usage, but lose outgoing messages when an adapter is lagging. Default: 10.
    pub rust_channel_size: usize,
    /// Enable sled.rs storage (disk + memory cache)? Default: true
    pub sled_storage: bool,
    /// Sled.rs config
    pub sled_config: sled::Config,
    /// Enable in-memory storage? Default: false
    pub memory_storage: bool,
    /// Enable multicast? Default: false
    pub multicast: bool, // should we have (adapters: Vector<String>) instead, so you can be sure there's no unwanted sync happening?
    /// Outgoing websocket peers. Urls must use wss: prefix, not https:. Default: empty list.
    pub outgoing_websocket_peers: Vec<String>,
    /// Run the websocket server? Default: `true`
    pub websocket_server: bool,
    /// Default: `4944`
    pub websocket_server_port: u16,
    /// Default: `8 * 1000 * 1000`
    pub websocket_frame_max_size: usize,
    /// TLS certificate path. Default: `None`
    pub cert_path: Option<String>,
    /// TLS key path. Default: `None`
    pub key_path: Option<String>,
    /// Show node stats at /stats?
    pub stats: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rust_channel_size: 1000,
            sled_storage: true,
            sled_config: sled::Config::new().path("sled_db"),
            memory_storage: false,
            multicast: false,
            outgoing_websocket_peers: Vec::new(),
            websocket_server: true,
            websocket_server_port: 4944,
            websocket_frame_max_size: 8 * 1000 * 1000,
            cert_path: None,
            key_path: None,
            stats: true,
        }
    }
}

/// A Graph Node that provides an API for graph traversal
/// Sends, processes and relays Put & Get messages between storage and transport adapters.
///
/// Supports graph synchronization over [Actor]s (currently websocket and multicast).
/// Disk storage adapter to be done.
#[derive(Clone)]
pub struct Node {
    config: Arc<RwLock<Config>>,
    uid: Arc<RwLock<String>>,
    path: Vec<String>,
    children: Arc<RwLock<BTreeMap<String, Node>>>,
    parents: Arc<RwLock<BTreeMap<String, Node>>>,
    on_sender: broadcast::Sender<GunValue>,
    map_sender: broadcast::Sender<(String, GunValue)>,
    actor_context: ActorContext,
    addr: Arc<RwLock<Option<Addr>>>,
    router: Arc<RwLock<Option<Addr>>>,
}

impl Node {
    /// Create a new root-level Node using default configuration.
    pub fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    /// Create a new root-level Node using custom configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// tokio_test::block_on(async {
    ///
    ///     use gundb::{Node, Config};
    ///     use gundb::types::GunValue;
    ///
    ///     let mut db = Node::new_with_config(Config {
    ///         outgoing_websocket_peers: vec!["wss://some-server-to-sync.with/gun".to_string()],
    ///         ..Config::default()
    ///     });
    ///     let mut sub = db.get("greeting").on();
    ///     db.get("greeting").put("Hello World!".into());
    ///     if let GunValue::Text(str) = sub.recv().await.unwrap() {
    ///         assert_eq!(&str, "Hello World!");
    ///     }
    ///
    /// })
    /// ```
    pub fn new_with_config(config: Config) -> Self {
        let mut node = Self {
            path: vec![],
            uid: Arc::new(RwLock::new("".to_string())),
            config: Arc::new(RwLock::new(config.clone())),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parents: Arc::new(RwLock::new(BTreeMap::new())),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            addr: Arc::new(RwLock::new(None)), // set this to None when stopping
            router: Arc::new(RwLock::new(None)),
            actor_context: ActorContext::new(random_string(16))
        };

        let (incoming_tx, incoming_rx) = unbounded_channel::<Message>();
        let addr = Addr::new(incoming_tx);
        let config = node.config.read().unwrap().clone();
        let router = Box::new(Router::new(config));
        let router_addr = node.actor_context.start_router(router);
        *node.router.write().unwrap() = Some(router_addr);
        *node.addr.write().unwrap() = Some(addr);
        node.listen(incoming_rx);

        node
    }

    fn handle_put(&mut self, msg: Put) {
        // notify subscriptions
    }

    fn listen(&mut self, mut receiver: UnboundedReceiver<Message>) {
        let mut clone = self.clone();
        self.actor_context.abort_on_stop(tokio::spawn(async move {
            while let Some(msg) = receiver.recv().await { // TODO shutdown
                debug!("incoming message");
                match msg {
                    Message::Put(put) => clone.handle_put(put),
                    _ => {}
                }
            }
        }));
    }

    fn new_child(&self, key: String) -> Node {
        assert!(key.len() > 0, "Key length must be greater than zero");
        debug!("new child {}", key);
        let mut parents = BTreeMap::new();
        parents.insert(self.uid.read().unwrap().clone(), self.clone());
        let config = self.config.read().unwrap();
        let mut path = self.path.clone();
        path.push(key.clone());
        let new_child_uid = path.join("/");
        debug!("new_child_uid {}", new_child_uid);
        let (sender, receiver) = unbounded_channel::<Message>();
        let mut node = Self {
            path,
            config: self.config.clone(),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parents: Arc::new(RwLock::new(parents)),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            uid: Arc::new(RwLock::new(new_child_uid)),
            router: self.router.clone(),
            addr: Arc::new(RwLock::new(Some(Addr::new(sender)))),
            actor_context: self.actor_context.clone()
        };
        node.listen(receiver);
        self.children.write().unwrap().insert(key, node.clone());
        node
    }

    /// Subscribe to the Node's value.
    pub fn on(&mut self) -> broadcast::Receiver<GunValue> {
        let key;
        if self.path.len() > 1 {
            key = self.path.iter().nth(self.path.len() - 1).cloned();
        } else {
            key = None;
        }
        let addr = self.addr.read().unwrap().clone();
        let get = Get::new(self.uid.read().unwrap().to_string(), key, addr.unwrap());
        if let Some(router) = self.router.read().unwrap().clone() {
            let _ = router.sender.send(Message::Get(get));
        }
        let sub = self.on_sender.subscribe();
        sub
    }

    // TODO: optionally specify which adapters to ask
    /// Return a child Node corresponding to the given Key.
    pub fn get(&mut self, key: &str) -> Node {
        if key == "" {
            return self.clone();
        }
        debug!("get key {}", key);
        if self.children.read().unwrap().contains_key(key) {
            self.children.read().unwrap().get(key).unwrap().clone() // TODO: theoretically, key could have been removed?
        } else {
            self.new_child(key.to_string())
        }
    }

    /// Subscribe to all children of this Node.
    pub fn map(&self) -> broadcast::Receiver<(String, GunValue)> {
        /*
        for (key, child) in self.children.read().unwrap().iter() { // TODO can be faster with rayon multithreading?
            if let Some(child_data) = child.clone().get_stored_data() {
                self.map_sender.send((key.to_string(), child_data.value)).ok(); // TODO first return Receiver and then do this in another thread?
            }
        }
         */
        self.map_sender.subscribe()
        // TODO: send get messages to adapters!!
    }

    /// Set a GunValue for the Node.
    pub fn put(&mut self, value: GunValue) {
        let updated_at: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as f64;
        let uid = self.uid.read().unwrap().clone();
        self.on_sender.send(value.clone()).ok();

        // TODO: write the full chain of parents
        for (parent_id, _parent) in self.parents.read().unwrap().iter() {
            let mut children = Children::default();
            children.insert(self.path.last().unwrap().clone(), NodeData { value: value.clone(), updated_at });
            let my_addr = self.addr.read().unwrap().clone();
            let put = Put::new_from_kv(parent_id.to_string(), children, my_addr.unwrap());
            if let Some(router) = &*self.router.read().unwrap() {
                let _ = router.sender.send(Message::Put(put));
            }
        }
    }

    pub fn stop(&mut self) {
        info!("Node stopping");
        self.actor_context.stop();
    }
}

#[cfg(test)]
mod tests {
    use crate::{Node, Config};
    use crate::types::GunValue;
    use tokio::time::{sleep, Duration};
    use std::sync::Once;
    //use log::{debug};

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            env_logger::init();
        });
    }

    // TODO proper test
    // TODO test .map()
    // TODO benchmark
    #[test]
    fn it_doesnt_error() {
        //setup();
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
        //setup();
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
        sleep(Duration::from_millis(1000)).await;
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
