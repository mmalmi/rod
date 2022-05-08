use std::collections::{BTreeMap, HashSet, HashMap};
use std::time::{SystemTime, Instant};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock // TODO: could we use async RwLock? Would require some changes to the chaining api (.get()).
};
use crate::utils::random_string;
use crate::router::Router;
use crate::message::{Message, Put, Get};
use crate::types::*;
use crate::adapters::SledStorage;
use crate::adapters::MemoryStorage;
use crate::adapters::WebsocketServer;
use crate::adapters::WebsocketClient;
use crate::adapters::Multicast;
use log::{debug, error};
use tokio::time::{sleep, Duration};
use tokio::sync::{broadcast, mpsc};
use sysinfo::{ProcessorExt, System, SystemExt};

// TODO extract networking to struct Mesh
// TODO proper automatic tests
// Node { node: Arc<RwLock<NodeInner>> } instead of Arc<RwLock> for each member? compare performance
// TODO connections don't seem to be closed / timeouted properly when client has disconnected
// TODO should use async RwLock everywhere?

// TODO: separate configs for each adapter?
/// [Node] configuration object.
#[derive(Clone)]
pub struct NodeConfig {
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

struct SeenGetMessage {
    from: String,
    last_reply_checksum: Option<String>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
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
/// Supports graph synchronization over [NetworkAdapter]s (currently websocket and multicast).
/// Disk storage adapter to be done.
#[derive(Clone)]
pub struct Node {
    pub config: Arc<RwLock<NodeConfig>>,
    uid: Arc<RwLock<String>>,
    path: Vec<String>,
    children: Arc<RwLock<BTreeMap<String, Node>>>,
    parents: Arc<RwLock<BTreeMap<String, Node>>>,
    on_sender: broadcast::Sender<GunValue>,
    map_sender: broadcast::Sender<(String, GunValue)>,
    stop_signal_sender: broadcast::Sender<()>,
    stop_signal_receiver: Arc<broadcast::Receiver<()>>,
    addr: Arc<RwLock<Option<Addr>>>,
    router: Arc<RwLock<Option<Addr>>>
}

impl Node {
    /// Create a new root-level Node using default configuration.
    pub fn new() -> Self {
        Self::new_with_config(NodeConfig::default())
    }

    /// Create a new root-level Node using custom configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// tokio_test::block_on(async {
    ///
    ///     use gundb::{Node, NodeConfig};
    ///     use gundb::types::GunValue;
    ///
    ///     let mut db = Node::new_with_config(NodeConfig {
    ///         outgoing_websocket_peers: vec!["wss://some-server-to-sync.with/gun".to_string()],
    ///         ..NodeConfig::default()
    ///     });
    ///     let mut sub = db.get("greeting").on();
    ///     db.get("greeting").put("Hello World!".into());
    ///     if let GunValue::Text(str) = sub.recv().await.unwrap() {
    ///         assert_eq!(&str, "Hello World!");
    ///     }
    ///
    /// })
    /// ```
    pub fn new_with_config(config: NodeConfig) -> Self {
        // Actix - needed or not? Maybe Node can be the System (entry point) of our Actor system?
        let stop_signal_channel = broadcast::channel::<()>(1);
        let outgoing_channel = broadcast::channel::<Message>(config.rust_channel_size);

        let node = Self {
            path: vec![],
            uid: Arc::new(RwLock::new("".to_string())),
            config: Arc::new(RwLock::new(config.clone())),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parents: Arc::new(RwLock::new(BTreeMap::new())),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            stop_signal_sender: stop_signal_channel.0,
            stop_signal_receiver: Arc::new(stop_signal_channel.1),
            addr: Arc::new(RwLock::new(None)), // set this to None when stopping
            router: Arc::new(RwLock::new(None)),
        };
        if config.multicast {
            let multicast = Multicast::new(node.clone());
            node.adapters.write().unwrap().insert("multicast".to_string(), Box::new(multicast));
        }
        if config.websocket_server {
            let server = WebsocketServer::new(node.clone());
            node.adapters.write().unwrap().insert("ws_server".to_string(), Box::new(server));
        }
        if config.sled_storage {
            let sled_storage = SledStorage::new(node.clone());
            node.adapters.write().unwrap().insert("sled_storage".to_string(), Box::new(sled_storage));
        }
        if config.memory_storage {
            let memory_storage = MemoryStorage::new(node.clone());
            node.adapters.write().unwrap().insert("memory_storage".to_string(), Box::new(memory_storage));
        }
        let client = WebsocketClient::new(node.clone());
        node.adapters.write().unwrap().insert("ws_client".to_string(), Box::new(client));
        node
    }

    /// NetworkAdapters should use this to receive outbound messages.
    pub fn get_outgoing_msg_receiver(&self) -> broadcast::Receiver<Message> {
        self.outgoing_msg_sender.subscribe()
    }

    /// NetworkAdapters should use this to relay inbound messages.
    pub fn get_incoming_msg_sender(&self) -> mpsc::Sender<Message> {
        self.incoming_msg_sender.read().unwrap().as_ref().unwrap().clone()
    }

    // record subscription & relay
    fn handle_get(&mut self, msg: Get) {
        if !msg.id.chars().all(char::is_alphanumeric) {
            error!("id {}", msg.id);
        }
        if self.is_message_seen(msg.id) {
            return;
        }
        let seen_get_message = SeenGetMessage { from: msg.from.clone(), last_reply_checksum: None };
        self.seen_get_messages.write().unwrap().insert(msg.id.clone(), seen_get_message);
        let topic = msg.node_id.split("/").next().unwrap_or("");
        debug!("{} subscribed to {}", msg.from, topic);
        self.subscribers_by_topic.write().unwrap().entry(topic.to_string())
            .or_insert_with(HashSet::new).insert(msg.from.clone());
        let id = msg.id.clone();
        self.send(Message::Get(msg), id);
    }

    // relay to original requester or all subscribers
    fn handle_put(&mut self, msg: Put) {
        if self.is_message_seen(msg.id) {
            return;
        }
        let mut recipients = HashSet::<String>::new();

        match &msg.in_response_to {
            Some(in_response_to) => {
                if let Some(seen_get_message) = self.seen_get_messages.write().unwrap().get_mut(in_response_to) {
                    if msg.checksum != None && msg.checksum == seen_get_message.last_reply_checksum {
                        debug!("same reply already sent");
                        return;
                    } // failing these conditions, should we still send the ack to someone?
                    seen_get_message.last_reply_checksum = msg.checksum.clone();
                    recipients.insert(seen_get_message.from.clone());
                }
            },
            _ => {
                for node_id in msg.updated_nodes.keys() {
                    let topic = node_id.split("/").next().unwrap_or("");
                    if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
                        recipients.extend(subscribers.clone());
                    }
                    debug!("getting subscribers for topic {}: {:?}", topic, recipients);
                }
            }
        };
        let mut msg = msg.clone();
        msg.recipients = Some(recipients);
        let id = msg.id.clone();
        self.send(Message::Put(msg), id);
    }

    // should we start adapters on ::new()? in a new thread
    /// Starts [NetworkAdapter]s that are enabled for this Node.
    pub async fn start(&mut self) {
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<Message>(self.config.read().unwrap().rust_channel_size);
        let my_addr = Addr::new(incoming_tx);
        let router = Router::new_with_config(config.clone());
        self.router = router.start();
        *self.addr.write().unwrap() = Some(addr);
        let mut node = self.clone();
        tokio::task::spawn(async move {
            loop {
                if let Some(msg) = incoming_rx.recv().await {
                    debug!("incoming message");
                    match msg {
                        Message::Put(put) => node.handle_put(put),
                        Message::Get(get) => node.handle_get(get),
                        _ => {}
                    }
                }
            }
        });

        let adapters = self.adapters.read().unwrap();
        let mut futures = Vec::new();
        for adapter in adapters.values() {
            futures.push(adapter.start()); // adapters must be non-blocking: use async functions or spawn_blocking
        }
        if self.config.read().unwrap().stats {
            self.update_stats();
        }

        let joined = futures::future::join_all(futures);

        futures::future::select(joined, Box::pin(self.stop_signal_sender.subscribe().recv())).await;
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
        let node = Self {
            path,
            config: self.config.clone(),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parents: Arc::new(RwLock::new(parents)),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            stop_signal_sender: self.stop_signal_sender.clone(),
            stop_signal_receiver: self.stop_signal_receiver.clone(),
            uid: Arc::new(RwLock::new(new_child_uid)),
            router: self.router.clone()
        };
        self.children.write().unwrap().insert(key, node.clone());
        node
    }

    /// Get the network peer id of this Node
    pub fn get_peer_id(&self) -> String {
        self.peer_id.read().unwrap().to_string()
    }

    /// Subscribe to the Node's value.
    pub fn on(&mut self) -> broadcast::Receiver<GunValue> {
        if self.adapters.read().unwrap().len() > 0 {
            let key;
            if self.path.len() > 1 {
                key = self.path.iter().nth(self.path.len() - 1).cloned();
            } else {
                key = None;
            }
            let get = Get::new(self.uid.read().unwrap().to_string(), key, self.get_peer_id().clone());
            let id = get.id.clone();
            self.outgoing_message(Message::Get(get), id);
        }
        let sub = self.on_sender.subscribe();
        let uid = self.uid.read().unwrap().clone();
        self.subscriptions_by_node_id.write().unwrap().entry(uid).or_insert_with(Vec::new).push(self.on_sender.clone());
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
        if self.adapters.read().unwrap().len() > 0 {
            // TODO: write the full chain of parents
            for (parent_id, _parent) in self.parents.read().unwrap().iter() {
                let mut children = Children::default();

                children.insert(self.path.last().unwrap().clone(), NodeData { value: value.clone(), updated_at });
                let mut put = Put::new_from_kv(parent_id.to_string(), children);
                put.from = self.get_peer_id();
                let recipients;
                let topic = self.path.iter().next().unwrap();
                debug!("getting subscribers for topic {}", topic);
                if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
                    recipients = subscribers.clone();
                } else {
                    recipients = HashSet::new();
                }
                put.recipients = Some(recipients);
                let id = put.id.clone();
                self.outgoing_message(Message::Put(put), id);
            }
        }
    }

    /// Stop any running adapters
    pub fn stop(&mut self) {
        let _ = self.stop_signal_sender.send(());
    }
}

#[cfg(test)]
mod tests {
    use crate::{Node, NodeConfig};
    use crate::types::GunValue;
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
    #[test]
    fn it_doesnt_error() {
        //setup();
        let mut gun = Node::new_with_config(NodeConfig {
            memory_storage: true,
            sled_storage: false,
            ..NodeConfig::default()
        });
        let _ = gun.get("Meneldor"); // Pick Tolkien names from https://www.behindthename.com/namesakes/list/tolkien/alpha
    }

    #[tokio::test]
    async fn first_get_then_put() {
        setup();
        let mut gun = Node::new_with_config(NodeConfig {
            memory_storage: true,
            sled_storage: false,
            ..NodeConfig::default()
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
        let mut gun = Node::new_with_config(NodeConfig {
            memory_storage: true,
            sled_storage: false,
            ..NodeConfig::default()
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
        let mut node1 = Node::new_with_config(NodeConfig {
            memory_storage: true,
            sled_storage: false,
            websocket_server: true,
            multicast: false,
            stats: false,
            ..NodeConfig::default()
        });
        let mut node2 = Node::new_with_config(NodeConfig {
            memory_storage: true,
            sled_storage: false,
            websocket_server: false,
            multicast: false,
            stats: false,
            outgoing_websocket_peers: vec!["ws://localhost:4944/gun".to_string()],
            ..NodeConfig::default()
        });
        async fn tst(mut node1: Node, mut node2: Node) {
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
        let node1_clone = node1.clone();
        let node2_clone = node2.clone();
        tokio::join!(node1.start_adapters(), node2.start_adapters(), tst(node1_clone, node2_clone));
    }

    /*
    #[tokio::test]
    async fn sync_over_multicast() {
        let mut node1 = Node::new_with_config(NodeConfig {
            websocket_server: false,
            multicast: true,
            stats: false,
            ..NodeConfig::default()
        });
        let mut node2 = Node::new_with_config(NodeConfig {
            websocket_server: false,
            multicast: true,
            stats: false,
            ..NodeConfig::default()
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
        tokio::join!(node1.start_adapters(), node2.start_adapters(), tst(node1_clone, node2_clone));
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
