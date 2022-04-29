use std::collections::{BTreeMap, HashSet, HashMap};
use std::time::{SystemTime, Instant};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock // TODO: could we use async RwLock? Would require some changes to the chaining api (.get()).
};
use crate::utils::random_string;
use crate::message::{Message, Put, Get};
use crate::types::*;
use crate::adapters::MemoryStorage;
use crate::adapters::WebsocketServer;
use crate::adapters::WebsocketClient;
use crate::adapters::Multicast;
use log::{debug, error};
use tokio::time::{sleep, Duration};
use tokio::sync::{broadcast, mpsc};
use sysinfo::{ProcessorExt, System, SystemExt};

static SEEN_MSGS_MAX_SIZE: usize = 10000;

// TODO extract networking to struct Mesh
// TODO proper automatic tests
// TODO persist data by saving root node to indexedDB as serialized by serde?
// Node { node: Arc<RwLock<NodeInner>> } instead of Arc<RwLock> for each member? compare performance
// TODO connections don't seem to be closed / timeouted properly when client has disconnected
// TODO should use async RwLock everywhere?

// TODO: separate configs for each adapter?
/// [Node] configuration object.
#[derive(Clone)]
pub struct NodeConfig {
    /// [tokio::sync::broadcast] channel size for outgoing network messages. Smaller value may slightly reduce memory usage, but lose outgoing messages when an adapter is lagging. Default: 10.
    pub rust_channel_size: usize,
    /// Enable in-memory storage? Default: true
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
    last_reply_hash: String,
}

impl Default for NodeConfig {
    fn default() -> Self {
        NodeConfig {
            rust_channel_size: 1000,
            memory_storage: true,
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

// Confusing name? Node is both a graph node and a network node.
/// A Graph Node that provides an API for graph traversal and publish-subscribe.
/// Sends, processes and relays Put & Get messages between storage and transport adapters.
///
/// Supports graph synchronization over [NetworkAdapter]s (currently websocket and multicast).
/// Disk storage adapter to be done.
#[derive(Clone)]
pub struct Node {
    path: Vec<String>,
    uid: Arc<RwLock<String>>,
    children: Arc<RwLock<BTreeMap<String, Node>>>,
    parents: Arc<RwLock<BTreeMap<String, Node>>>,
    pub config: Arc<RwLock<NodeConfig>>,
    on_sender: broadcast::Sender<GunValue>,
    map_sender: broadcast::Sender<(String, GunValue)>,
    adapters: NetworkAdapters,
    seen_messages: Arc<RwLock<BoundedHashSet>>,
    seen_get_messages: Arc<RwLock<BoundedHashMap<String, SeenGetMessage>>>,
    subscribers_by_topic: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    peer_id: Arc<RwLock<String>>,
    msg_counter: Arc<AtomicUsize>,
    stop_signal_sender: broadcast::Sender<()>,
    stop_signal_receiver: Arc<broadcast::Receiver<()>>,
    outgoing_msg_sender: broadcast::Sender<Message>,
    outgoing_msg_receiver: Arc<broadcast::Receiver<Message>>, // need to store 1 receiver instance so the channel doesn't get closed
    incoming_msg_sender: Arc<RwLock<Option<mpsc::Sender<Message>>>>,
    subscriptions_by_node_id: Arc<RwLock<HashMap<String, Vec<broadcast::Sender<GunValue>>>>>
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
            adapters: NetworkAdapters::default(),
            seen_messages: Arc::new(RwLock::new(BoundedHashSet::new(SEEN_MSGS_MAX_SIZE))),
            seen_get_messages: Arc::new(RwLock::new(BoundedHashMap::new(SEEN_MSGS_MAX_SIZE))),
            subscribers_by_topic: Arc::new(RwLock::new(HashMap::new())),
            peer_id: Arc::new(RwLock::new(random_string(16))),
            msg_counter: Arc::new(AtomicUsize::new(0)),
            stop_signal_sender: stop_signal_channel.0,
            stop_signal_receiver: Arc::new(stop_signal_channel.1),
            outgoing_msg_sender: outgoing_channel.0,
            outgoing_msg_receiver: Arc::new(outgoing_channel.1),
            incoming_msg_sender: Arc::new(RwLock::new(None)),
            subscriptions_by_node_id: Arc::new(RwLock::new(HashMap::new()))
        };
        if config.multicast {
            let multicast = Multicast::new(node.clone());
            node.adapters.write().unwrap().insert("multicast".to_string(), Box::new(multicast));
        }
        if config.websocket_server {
            let server = WebsocketServer::new(node.clone());
            node.adapters.write().unwrap().insert("ws_server".to_string(), Box::new(server));
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

    fn update_stats(&self) {
        let mut node = self.clone();
        let peer_id = self.get_peer_id();
        let mut stats = node.get("node_stats").get(&peer_id);
        let start_time = Instant::now();
        tokio::task::spawn(async move {
            let mut sys = System::new_all();
            loop {
                sys.refresh_all();
                stats.get("msgs_per_second").put(node.msg_counter.load(Ordering::Relaxed).into());
                node.msg_counter.store(0, Ordering::Relaxed);
                stats.get("total_memory").put(format!("{} MB", sys.total_memory() / 1000).into());
                stats.get("used_memory").put(format!("{} MB", sys.used_memory() / 1000).into());
                stats.get("cpu_usage").put(format!("{} %", sys.global_processor_info().cpu_usage() as u64).into());
                let uptime_secs = start_time.elapsed().as_secs();
                let uptime;
                if uptime_secs <= 60 {
                    uptime = format!("{} seconds", uptime_secs);
                } else if uptime_secs <= 2 * 60 * 60 {
                    uptime = format!("{} minutes", uptime_secs / 60);
                } else {
                    uptime = format!("{} hours", uptime_secs / 60 / 60);
                }
                stats.get("process_uptime").put(uptime.into());
                sleep(Duration::from_millis(1000)).await;
            }
        });
    }

    // should we start adapters on ::new()? in a new thread
    /// Starts [NetworkAdapter]s that are enabled for this Node.
    pub async fn start_adapters(&mut self) {
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<Message>(self.config.read().unwrap().rust_channel_size);
        *self.incoming_msg_sender.write().unwrap() = Some(incoming_tx);
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
        debug!("new_child_uid {:?}", new_child_uid);
        let node = Self {
            path,
            config: self.config.clone(),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parents: Arc::new(RwLock::new(parents)),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            adapters: self.adapters.clone(),
            seen_messages: self.seen_messages.clone(),
            seen_get_messages: self.seen_get_messages.clone(),
            subscribers_by_topic: self.subscribers_by_topic.clone(),
            peer_id: self.peer_id.clone(),
            msg_counter: self.msg_counter.clone(),
            stop_signal_sender: self.stop_signal_sender.clone(),
            stop_signal_receiver: self.stop_signal_receiver.clone(),
            outgoing_msg_sender: self.outgoing_msg_sender.clone(),
            outgoing_msg_receiver: self.outgoing_msg_receiver.clone(),
            incoming_msg_sender: self.incoming_msg_sender.clone(),
            uid: Arc::new(RwLock::new(new_child_uid)),
            subscriptions_by_node_id: self.subscriptions_by_node_id.clone()
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
            let get = Get::new(self.uid.read().unwrap().to_string(), key);
            self.outgoing_message(Message::Get(get), get.id.clone());
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

    // record subscription & relay
    fn handle_get(&mut self, msg: Get) {
        if self.is_message_seen(msg.id.clone(), msg.to_string()) {
            return;
        }
        let topic = msg.node_id.split("/").next().unwrap_or("");
        debug!("{} subscribed to {}", from, topic);
        self.subscribers_by_topic.write().unwrap().entry(topic.to_string())
            .or_insert_with(HashSet::new).insert(from.clone());
        self.outgoing_message(Message::Get(msg), msg.id.clone());
    }

    // relay to original requester or all subscribers
    fn handle_put(&mut self, msg: Put) {
        if self.is_message_seen(msg.id.clone(), msg.to_string()) {
            return;
        }
        let mut recipients = HashSet::<String>::new();
        if let Some(in_response_to) = msg.in_response_to {
            let mut content_hash = "-".to_string();
            if let Some(hash) = msg_obj.get("##") {
                content_hash = hash.to_string();
            }
            let in_response_to = in_response_to.to_string();
            if let Some(seen_get_message) = self.seen_get_messages.write().unwrap().get_mut(&in_response_to) {
                if content_hash == seen_get_message.last_reply_hash {
                    debug!("same reply already sent");
                    return;
                } // failing these conditions, should we still send the ack to someone?
                seen_get_message.last_reply_hash = content_hash;
                recipients.insert(seen_get_message.from.clone());
            }
        } else {
            for node_id in msg.updated_nodes.keys() {
                let topic = node_id.split("/").next().unwrap_or("");
                debug!("getting subscribers for topic {}", topic);
                if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
                    recipients.extend(subscribers.clone());
                }
            }
        }
        let mut msg = msg.clone();
        msg.recipients = Some(recipients);
        self.outgoing_message(Message::Put(msg), msg.id.clone());
    }

    fn is_message_seen(&mut self, id: String, msg_str: String) -> bool {
        self.msg_counter.fetch_add(1, Ordering::Relaxed);

        if self.seen_messages.read().unwrap().contains(&id) {
            debug!("already seen message {}", &id);
            return true;
        }
        self.seen_messages.write().unwrap().insert(id.clone());

        let s: String = msg_str.chars().take(300).collect();
        debug!("in ID {}:\n{}\n", id.to_string(), s);
        return false;
    }

    fn outgoing_message(&self, msg: Message, msg_id: String) {
        debug!("[{}] msg out {:?}", &self.get_peer_id()[..4], msg.to_string());
        self.seen_messages.write().unwrap().insert(msg_id); // TODO: doesn't seem to work, at least on multicast
        if let Err(e) = self.outgoing_msg_sender.send(msg) {
            error!("failed to send outgoing message from node: {}", e);
        };
    }

    /// Set a GunValue for the Node.
    pub fn put(&mut self, value: GunValue) {
        let updated_at: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as f64;
        // TODO: move in-memory storage into an adapter
        let uid = self.uid.read().unwrap().clone();
        // TODO: save parents, or move self.store to adapter
        self.on_sender.send(value.clone()).ok();
        if self.adapters.read().unwrap().len() > 0 {
            let mut put = Put::new_from_kv(uid, NodeData { value, updated_at });
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
            self.outgoing_message(Message::Put(put), put.id.clone());
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
        let mut gun = Node::new();
        let _ = gun.get("Meneldor"); // Pick Tolkien names from https://www.behindthename.com/namesakes/list/tolkien/alpha
    }

    #[tokio::test]
    async fn first_get_then_put() {
        setup();
        let mut gun = Node::new();
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
        let mut gun = Node::new();
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
            websocket_server: true,
            multicast: false,
            stats: false,
            ..NodeConfig::default()
        });
        let mut node2 = Node::new_with_config(NodeConfig {
            websocket_server: false,
            multicast: false,
            stats: false,
            outgoing_websocket_peers: vec!["ws://localhost:4944/gun".to_string()],
            ..NodeConfig::default()
        });
        async fn tst(mut node1: Node, mut node2: Node) {
            sleep(Duration::from_millis(1000)).await;
            let mut sub1 = node1.get("node2").on();
            let mut sub2 = node2.get("node1").on();
            node1.get("node1").put("Amandil".into());
            node2.get("node2").put("Beregond".into());
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
