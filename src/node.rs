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
    /// [tokio::sync::broadcast] channel size for Node::map() and on(). Default: 10
    pub rust_channel_size: usize,
    /// Enable sled.rs storage (disk + memory cache)? Default: true
    pub sled_storage: bool,
    /// Sled.rs config. Default: sled::Config defaults + db path "sled_db".
    pub sled_config: sled::Config,
    /// Limit for the sled database size on disk in bytes. Default: None
    pub sled_storage_limit: Option<u64>,
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
    /// Prioritize data storage for this Gun public key. Format: x.y where x and y are base64 encoded ECDSA public key coordinates.
    /// Example: hyECQHwSo7fgr2MVfPyakvayPeixxsaAWVtZ-vbaiSc.TXIp8MnCtrnW6n2MrYquWPcc-DTmZzMBmc2yaGv9gIU
    pub my_pub: Option<String>,
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
            sled_storage_limit: None,
            memory_storage: false,
            multicast: false,
            outgoing_websocket_peers: Vec::new(),
            websocket_server: true,
            websocket_server_port: 4944,
            websocket_frame_max_size: 8 * 1000 * 1000,
            cert_path: None,
            key_path: None,
            stats: true,
            my_pub: None,
        }
    }
}

/// A Graph Node that provides an API for graph traversal.
/// Sends, processes and relays Put & Get messages between storage and transport adapters.
#[derive(Clone)]
pub struct Node {
    config: Arc<RwLock<Config>>,
    uid: Arc<RwLock<String>>,
    path: Vec<String>,
    children: Arc<RwLock<BTreeMap<String, Node>>>,
    parent: Arc<RwLock<Option<(String, Node)>>>,
    on_sender: broadcast::Sender<GunValue>,
    map_sender: broadcast::Sender<(String, GunValue)>,
    actor_context: ActorContext,
    addr: Arc<RwLock<Addr>>,
    router: Arc<RwLock<Option<Addr>>>,
}

impl Node {
    /// Create a new root-level Node using default configuration. Starts the default network and storage adapters.
    pub fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    /// Create a new root-level Node using custom configuration. Starts the default or configured network and storage adapters.
    ///
    /// # Examples
    ///
    /// ```
    /// tokio_test::block_on(async {
    ///
    ///     use gundb::{Node, Config, GunValue};
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
        let (incoming_tx, incoming_rx) = unbounded_channel::<Message>();
        let addr = Addr::new(incoming_tx);
        let mut node = Self {
            path: vec![],
            uid: Arc::new(RwLock::new("".to_string())),
            config: Arc::new(RwLock::new(config.clone())),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parent: Arc::new(RwLock::new(None)),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            addr: Arc::new(RwLock::new(addr)),
            router: Arc::new(RwLock::new(None)),
            actor_context: ActorContext::new(random_string(16))
        };

        let router = Box::new(Router::new(config.clone())); // actually, we should communicate with
        // MemoryStorage, which has a special role in maintaining our version of the current state?
        // MemoryStorage can then communicate with router as needed.
        let router_addr = node.actor_context.start_router(router);
        *node.router.write().unwrap() = Some(router_addr);
        node.listen(incoming_rx);

        node
    }

    fn handle_put(&mut self, put: Put) {
        // TODO accept puts only from our memory adapter, which is supposed to serve the latest version.
        // Or store latest NodeData in Node?
        for (node_id, node_data) in put.updated_nodes {
            if node_id == *self.uid.read().unwrap() {
                for (child, child_data) in node_data {
                    if let Some(child) = self.children.read().unwrap().get(&child) {
                        let _ = child.on_sender.send(child_data.value.clone());
                    }
                    let _ = self.map_sender.send((child.to_string(), child_data.value.clone()));
                }
            }
        }
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
            parent: Arc::new(RwLock::new(Some((self.uid.read().unwrap().clone(), self.clone())))),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            uid: Arc::new(RwLock::new(new_child_uid)),
            router: self.router.clone(),
            addr: Arc::new(RwLock::new(Addr::new(sender))),
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
        let addr;
        let node_id;
        if let Some((parent_id, parent)) = &*self.parent.read().unwrap() {
            node_id = parent_id.clone();
            addr = parent.addr.read().unwrap().clone();
        } else {
            node_id = self.uid.read().unwrap().to_string();
            addr = self.addr.read().unwrap().clone();
        }
        let get = Get::new(node_id, key, addr);
        if let Some(router) = self.router.read().unwrap().clone() {
            let _ = router.sender.send(Message::Get(get));
        }
        self.on_sender.subscribe()
    }

    // TODO: optionally specify which adapters to ask
    /// Return a child Node corresponding to the given key.
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
        self.on_sender.send(value.clone()).ok();
        debug!("put {}", value.to_string());

        // TODO: write the full chain of parents

        if let Some((parent_id, _parent)) = &*self.parent.read().unwrap() {
            let mut children = Children::default();
            children.insert(self.path.last().unwrap().clone(), NodeData { value: value.clone(), updated_at });
            let my_addr = self.addr.read().unwrap().clone();
            let put = Put::new_from_kv(parent_id.to_string(), children, my_addr);
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
