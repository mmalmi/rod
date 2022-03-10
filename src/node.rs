use std::collections::{BTreeMap, HashSet, HashMap};
use std::time::{SystemTime, Instant};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock // TODO: could we use async RwLock? Would require some changes to the chaining api (.get()).
};
use serde_json::{json, Value as SerdeJsonValue};
use crate::types::*;
use crate::utils::random_string;
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

/// A Graph Node that provides an API for graph traversal and publish-subscribe.
///
/// Supports graph synchronization over [NetworkAdapter]s (currently websocket and multicast).
/// Disk storage adapter to be done.
#[derive(Clone)]
pub struct Node {
    id: String,
    pub config: Arc<RwLock<NodeConfig>>,
    graph_size_bytes: Arc<RwLock<usize>>,
    updated_at: Arc<RwLock<f64>>, // TODO: Option<f64>?
    key: String,
    path: Vec<String>,
    value: Value,
    children: Children,
    parents: Parents,
    on_sender: broadcast::Sender<GunValue>,
    map_sender: broadcast::Sender<(String, GunValue)>,
    store: SharedNodeStore, // experimental: replace this with redis?
    network_adapters: NetworkAdapters,
    seen_messages: Arc<RwLock<BoundedHashSet>>,
    seen_get_messages: Arc<RwLock<BoundedHashMap<String, SeenGetMessage>>>,
    subscribers_by_topic: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    peer_id: Arc<RwLock<String>>,
    msg_counter: Arc<AtomicUsize>,
    stop_signal_sender: broadcast::Sender<()>,
    stop_signal_receiver: Arc<broadcast::Receiver<()>>,
    outgoing_msg_sender: broadcast::Sender<GunMessage>,
    outgoing_msg_receiver: Arc<broadcast::Receiver<GunMessage>>, // need to store 1 receiver instance so the channel doesn't get closed
    incoming_msg_sender: Arc<RwLock<Option<mpsc::Sender<GunMessage>>>>
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
        let outgoing_channel = broadcast::channel::<GunMessage>(config.rust_channel_size);
        let node = Self {
            id: "".to_string(),
            config: Arc::new(RwLock::new(config.clone())),
            graph_size_bytes: Arc::new(RwLock::new(0)),
            updated_at: Arc::new(RwLock::new(0.0)),
            key: "".to_string(),
            path: Vec::new(),
            value: Value::default(),
            children: Children::default(),
            parents: Parents::default(),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            store: SharedNodeStore::default(),
            network_adapters: NetworkAdapters::default(),
            seen_messages: Arc::new(RwLock::new(BoundedHashSet::new(SEEN_MSGS_MAX_SIZE))),
            seen_get_messages: Arc::new(RwLock::new(BoundedHashMap::new(SEEN_MSGS_MAX_SIZE))),
            subscribers_by_topic: Arc::new(RwLock::new(HashMap::new())),
            peer_id: Arc::new(RwLock::new(random_string(16))),
            msg_counter: Arc::new(AtomicUsize::new(0)),
            stop_signal_sender: stop_signal_channel.0,
            stop_signal_receiver: Arc::new(stop_signal_channel.1),
            outgoing_msg_sender: outgoing_channel.0,
            outgoing_msg_receiver: Arc::new(outgoing_channel.1),
            incoming_msg_sender: Arc::new(RwLock::new(None))
        };
        if config.multicast {
            let multicast = Multicast::new(node.clone());
            node.network_adapters.write().unwrap().insert("multicast".to_string(), Box::new(multicast));
        }
        if config.websocket_server {
            let server = WebsocketServer::new(node.clone());
            node.network_adapters.write().unwrap().insert("ws_server".to_string(), Box::new(server));
        }
        let client = WebsocketClient::new(node.clone());
        node.network_adapters.write().unwrap().insert("ws_client".to_string(), Box::new(client));
        node
    }

    /// NetworkAdapters should use this to receive outbound messages.
    pub fn get_outgoing_msg_receiver(&self) -> broadcast::Receiver<GunMessage> {
        self.outgoing_msg_sender.subscribe()
    }

    /// NetworkAdapters should use this to receive outbound messages.
    pub fn get_incoming_msg_sender(&self) -> mpsc::Sender<GunMessage> {
        self.incoming_msg_sender.read().unwrap().as_ref().unwrap().clone()
    }

    fn update_stats(&self) {
        let mut node = self.clone();
        let peer_id = node.peer_id.read().unwrap().to_string();
        let start_time = Instant::now();
        tokio::task::spawn(async move {
            let mut sys = System::new_all();
            loop {
                sys.refresh_all();
                let count = node.store.read().unwrap().len().to_string();
                let graph_size_bytes = *node.graph_size_bytes.read().unwrap();
                let graph_size_bytes = format!("{}B", size_format::SizeFormatterBinary::new(graph_size_bytes as u64).to_string());
                let mut stats = node.get("node_stats").get(&peer_id);
                stats.get("msgs_per_second").put(node.msg_counter.load(Ordering::Relaxed).into());
                node.msg_counter.store(0, Ordering::Relaxed);
                stats.get("graph_node_count").put(count.into());
                stats.get("graph_size_bytes").put(graph_size_bytes.into());
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
        let (incoming_tx, mut incoming_rx) = mpsc::channel::<GunMessage>(self.config.read().unwrap().rust_channel_size);
        *self.incoming_msg_sender.write().unwrap() = Some(incoming_tx);
        let mut node = self.clone();
        tokio::task::spawn(async move {
            loop {
                if let Some(gun_message) = incoming_rx.recv().await {
                    debug!("incoming message");
                    node.incoming_message(gun_message.msg, &gun_message.from);
                }
            }
        });

        let adapters = self.network_adapters.read().unwrap();
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

    fn new_child(&self, key: String) -> String {
        assert!(key.len() > 0, "Key length must be greater than zero");
        debug!("new child {} {}", self.path.join("/"), key);
        let mut parents = HashSet::new();
        parents.insert((self.id.clone(), key.clone()));
        let mut path = self.path.clone();
        if self.key.len() > 0 {
            path.push(self.key.clone());
        }
        let config = self.config.read().unwrap();
        let mut path_clone = path.clone();
        path_clone.push(key.clone());
        let id = path_clone.join("/").to_string(); // random_string(16);
        let node = Self {
            id: id.clone(),
            config: self.config.clone(),
            graph_size_bytes: self.graph_size_bytes.clone(),
            updated_at: Arc::new(RwLock::new(0.0)),
            key: key.clone(),
            path,
            value: Value::default(),
            children: Children::default(),
            parents: Arc::new(RwLock::new(parents)),
            on_sender: broadcast::channel::<GunValue>(config.rust_channel_size).0,
            map_sender: broadcast::channel::<(String, GunValue)>(config.rust_channel_size).0,
            store: self.store.clone(),
            network_adapters: self.network_adapters.clone(),
            seen_messages: self.seen_messages.clone(),
            seen_get_messages: self.seen_get_messages.clone(),
            subscribers_by_topic: self.subscribers_by_topic.clone(),
            peer_id: self.peer_id.clone(),
            msg_counter: self.msg_counter.clone(),
            stop_signal_sender: self.stop_signal_sender.clone(),
            stop_signal_receiver: self.stop_signal_receiver.clone(),
            outgoing_msg_sender: self.outgoing_msg_sender.clone(),
            outgoing_msg_receiver: self.outgoing_msg_receiver.clone(),
            incoming_msg_sender: self.incoming_msg_sender.clone()
        };
        self.store.write().unwrap().insert(id.clone(), node);
        self.children.write().unwrap().insert(key, id.clone());
        id
    }

    /// Get the network peer id of this Node
    pub fn get_peer_id(&self) -> String {
        self.peer_id.read().unwrap().to_string()
    }

    /// Subscribe to the Node's value.
    pub fn on(&mut self) -> broadcast::Receiver<GunValue> {
        if self.network_adapters.read().unwrap().len() > 0 {
            let (m, id) = self.create_get_msg();
            self.outgoing_message(&m.to_string(), &self.get_peer_id(), id, None);
        }
        let sub = self.on_sender.subscribe();
        if let Some(val) = self.get_local_value_once() {
            self.on_sender.send(val).ok();
        }
        sub
    }

    // TODO: optionally specify which adapters to ask
    /// Return a child Node corresponding to the given Key.
    pub fn get(&mut self, key: &str) -> Node {
        if key == "" {
            return self.clone();
        }
        let id = self.get_child_id(key.to_string());
        let mut node = self.store.read().unwrap().get(&id).unwrap().clone();
        node.key = key.to_string();
        node
    }

    /// Subscribe to all children of this Node.
    pub fn map(&self) -> broadcast::Receiver<(String, GunValue)> {
        for (key, child_id) in self.children.read().unwrap().iter() { // TODO can be faster with rayon multithreading?
            if let Some(child) = self.store.read().unwrap().get(child_id) {
                if let Some(child_val) = child.clone().get_local_value_once() {
                    self.map_sender.send((key.to_string(), child_val)).ok(); // TODO first return Receiver and then do this in another thread?
                }
            }
        }
        self.map_sender.subscribe()
        // TODO: send get messages to adapters!!
    }

    fn get_child_id(&mut self, key: String) -> String {
        let existing_id = match self.children.read().unwrap().get(&key) {
            Some(node_id) => Some(node_id.clone()),
            _ => None
        };
        match existing_id {
            Some(id) => id,
            _ => self.new_child(key)
        }
    }

    fn create_get_msg(&self) -> (String, String) {
        let msg_id = random_string(8);
        let key = self.key.clone();
        let json;
        if self.path.len() > 0 {
            let path = self.path.join("/");
            json = json!({
                "get": {
                    "#": path,
                    ".": key
                },
                "#": msg_id
            }).to_string();
        } else {
            json = json!({
                "get": {
                    "#": key
                },
                "#": msg_id
            }).to_string();
        }
        (json, msg_id)
    }

    fn create_put_msg(&self, value: &GunValue, updated_at: f64) -> (String, String) {
        let msg_id = random_string(8);
        let full_path = &self.path.join("/");
        let key = &self.key.clone();
        let mut json = json!({
            "put": {
                full_path: { // soul, not path
                    "_": {
                        "#": full_path, // soul, not path
                        ">": {
                            key: updated_at
                        }
                    },
                    key: value // TODO: in case of Link / Children, this should be the Soul
                    // https://gun.eco/docs/FAQ#what-is-a-soul-what-does-a-node-look-like
                }
            },
            "#": msg_id,
        });

        let puts = &mut json["put"];
        // if it's a nested node, put its parents also
        for (i, node_name) in self.path.iter().enumerate().nth(1) {
            let path = self.path[..i].join("/");
            let path_obj = json!({
                "_": {
                    "#": path, // soul, not path
                    ">": {
                        node_name: updated_at
                    }
                },
                node_name: {
                    "#": self.path[..(i+1)].join("/") // this should be the soul? sea signed
                }
            });
            puts[path] = path_obj;
        }
        (json.to_string(), msg_id)
    }

    fn incoming_message_json(&mut self, msg: &SerdeJsonValue, is_from_array: bool, msg_str: Option<String>, from: &String) {
        if let Some(array) = msg.as_array() {
            if is_from_array {
                error!("received nested array {}", msg);
                return;
            } // don't allow array inside array
            for msg in array.iter() {
                self.incoming_message_json(msg, true, None, from);
            }
            return;
        }
        if let Some(msg_obj) = msg.as_object() {
            if let Some(msg_id) = msg_obj.get("#") {
                if let Some(msg_id) = msg_id.as_str() {
                    let msg_id = msg_id.to_string();
                    if self.seen_messages.read().unwrap().contains(&msg_id) {
                        debug!("already have {}", &msg_id);
                        return;
                    }
                    self.seen_messages.write().unwrap().insert(msg_id.clone());
                    let msg_str = match msg_str {
                        Some(s) => s,
                        None => msg.to_string()
                    };
                    let s: String = msg_str.chars().take(300).collect();
                    debug!("in ID {}:\n{}\n", msg_id, s);

                    if let Some(put) = msg_obj.get("put") {
                        if let Some(put_obj) = put.as_object() {
                            self.incoming_put(&msg_str, &msg_id, &from, msg_obj, put_obj);
                        }
                    }
                    if let Some(get) = msg_obj.get("get") {
                        if let Some(get_obj) = get.as_object() {
                            self.seen_get_messages.write().unwrap().insert(msg_id.clone(), SeenGetMessage {
                                from: from.clone(),
                                last_reply_hash: "".to_string()
                            });
                            self.incoming_get(get_obj, from);
                            self.outgoing_message(&msg_str, from, msg_id, None); // TODO: randomly sample recipients
                        }
                    }
                }
            } else {
                debug!("msg without id: {}\n", msg);
            }
        }
    }

    /// NetworkAdapters should call this for received messages.
    fn incoming_message(&mut self, msg: String, from: &String) {
        debug!("msg in {}", msg);
        self.msg_counter.fetch_add(1, Ordering::Relaxed);
        let json: SerdeJsonValue = match serde_json::from_str(&msg) {
            Ok(json) => json,
            Err(_) => { return; }
        };
        self.incoming_message_json(&json, false, Some(msg), from);
    }

    fn incoming_put(&mut self, msg_str: &String, msg_id: &String, from: &String, msg_obj: &serde_json::Map<String, SerdeJsonValue>, put_obj: &serde_json::Map<String, SerdeJsonValue>) {
        let mut recipients = HashSet::new();
        let mut is_ack = false;
        if let Some(in_response_to) = msg_obj.get("@") {
            if let Some(in_response_to) = in_response_to.as_str() {
                is_ack = true;
                let mut content_hash = "-".to_string();
                if let Some(hash) = msg_obj.get("##") {
                    content_hash = hash.to_string();
                }
                let in_response_to = in_response_to.to_string();
                if let Some(seen_get_message) = self.seen_get_messages.write().unwrap().get_mut(&in_response_to) {
                    if content_hash == seen_get_message.last_reply_hash {
                        return;
                    } // failing these conditions, should we still send the ack to someone?
                    seen_get_message.last_reply_hash = content_hash;
                    recipients.insert(seen_get_message.from.clone());
                }
            }
        }
        for (updated_key, update_data) in put_obj.iter() {
            if !is_ack {
                let topic = updated_key.split("/").next().unwrap_or("");
                debug!("getting subscribers for topic {}", topic);
                if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
                    recipients.extend(subscribers.clone());
                }
            }
            let mut node = self.clone();// = self.get(updated_key);
            for node_name in updated_key.split("/") {
                node = node.get(node_name);
            }
            if let Some(updated_at_times) = update_data["_"][">"].as_object() {
                for (child_key, incoming_val_updated_at) in updated_at_times.iter() {
                    if let Some(incoming_val_updated_at) = incoming_val_updated_at.as_f64() {
                        let mut child = node.get(child_key);
                        if *child.updated_at.read().unwrap() <= incoming_val_updated_at {
                            // TODO if incoming_val_updated_at > current_time { defer_operation() }
                            if let Some(new_value) = update_data.get(child_key) {
                                if let Ok(new_value) = serde_json::from_value::<GunValue>(new_value.clone()) {
                                    child.put_local(new_value, incoming_val_updated_at);
                                }
                            }
                        } // TODO else append to history
                    }
                }
            }
        }
        recipients.remove(from);
        self.outgoing_message(&msg_str, from, msg_id.clone(), Some(recipients));
    }

    fn _children_to_gun_value(&self, children: &BTreeMap<String, String>) -> GunValue {
        let mut map = BTreeMap::<String, GunValue>::new();
        for (key, child_id) in children.iter() { // TODO faster with rayon?
            let child_value: Option<GunValue> = match self.store.read().unwrap().get(child_id) {
                Some(child) => match &*(child.value.read().unwrap()) {
                    Some(value) => Some(value.clone()),
                    _ => None
                },
                _ => None
            };
            if let Some(value) = child_value {
                map.insert(key.clone(), value);
            } else { // return child Node object
                map.insert(key.clone(), GunValue::Link(child_id.clone()));
            }
        }
        GunValue::Children(map)
    }

    fn outgoing_message(&self, msg: &String, from: &String, msg_id: String, to: Option<HashSet<String>>) {
        //debug!("sending msg {} {}", &msg_id, msg);
        self.seen_messages.write().unwrap().insert(msg_id); // TODO: doesn't seem to work, at least on multicast
        if let Err(e) = self.outgoing_msg_sender.send(GunMessage { msg: msg.clone(), from: from.clone(), to }) {
            error!("failed to send outgoing message from node: {}", e);
        };
    }

    pub fn get_local_value_once(&self) -> Option<GunValue> {
        if let Some(value) = &*self.value.read().unwrap() {
            Some(value.clone())
        } else {
            let children = self.children.read().unwrap();
            if !children.is_empty() {
                let obj = self._children_to_gun_value(&children);
                return Some(obj)
            }
            None
        }
    }

    fn send_get_response_if_have(&self, from: &str) {
        if let Some(value) = self.get_local_value_once() {
            let msg_id = random_string(8);
            let full_path = &self.path.join("/");
            let key = &self.key.clone();
            let json = json!({
                "put": {
                    full_path: {
                        "_": {
                            "#": full_path,
                            ">": {
                                key: &*self.updated_at.read().unwrap()
                            }
                        },
                        key: value
                    }
                },
                "#": msg_id,
            }).to_string();
            debug!("have! sending response {}", msg_id);
            let mut recipients = HashSet::new();
            recipients.insert(from.to_string());
            self.outgoing_message(&json, &self.get_peer_id(), msg_id, Some(recipients)); // TODO: send only to the requester
        }
    }

    fn incoming_get(&mut self, get: &serde_json::Map<String, SerdeJsonValue>, from: &String) {
        if let Some(path) = get.get("#") {
            if let Some(path) = path.as_str() {
                {
                    let topic = path.split("/").next().unwrap_or("");
                    debug!("{} subscribed to {}", from, topic);
                    let mut subscribers_by_topic = self.subscribers_by_topic.write().unwrap();
                    if !subscribers_by_topic.contains_key(topic) {
                        subscribers_by_topic.insert(topic.to_string(), HashSet::new());
                    }
                    subscribers_by_topic.get_mut(topic.clone()).unwrap().insert(from.clone());
                }
                if let Some(key) = get.get(".") {
                    // debug!("yes . {} {}", path, key);
                    if let Some(key) = key.as_str() {
                        let mut split = path.split("/");
                        let mut node = self.get(split.nth(0).unwrap());
                        for node_name in split.nth(0) {
                            node = node.get(node_name); // TODO get only existing nodes in order to not spam our graph with empties
                        }
                        node = node.get(key);
                        node.send_get_response_if_have(from); // TODO don't send response to everyone
                    }
                } else {
                    let mut split = path.split("/");
                    let mut node = self.get(split.nth(0).unwrap());
                    for node_name in split.nth(0) {
                        node = node.get(node_name); // TODO get only existing nodes in order to not spam our graph with empties
                    }
                    node.send_get_response_if_have(from);

                    // debug!("no {}", path);
                }
            }
        }
    }

    /// Set a GunValue for the Node.
    pub fn put(&mut self, value: GunValue) {
        let time: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as f64;
        self.put_local(value.clone(), time);
        if self.network_adapters.read().unwrap().len() > 0 {
            let (m, id) = self.create_put_msg(&value, time);
            let recipients;
            let mut topic = "";
            if let Some(path) = self.path.get(0) {
                topic = path;
            }
            debug!("getting subscribers for topic {}", topic);
            if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
                recipients = subscribers.clone();
            } else {
                recipients = HashSet::new();
            }
            self.outgoing_message(&m, &self.get_peer_id(), id, Some(recipients));
        }
    }

    fn put_local(&mut self, value: GunValue, time: f64) {
        debug!("put_local\npath: {}\nkey: {}\nvalue: {:?}\n", self.path.join("/"), self.key, value);
        // root.get(soul).get(key).put(jsvalue)
        // TODO handle javascript Object values
        // TODO: if "children" is replaced with "value", remove backreference from linked objects

        *self.updated_at.write().unwrap() = time;
        *self.children.write().unwrap() = BTreeMap::new();
        *self.value.write().unwrap() = Some(value.clone());
        debug!("send to sender {:?}", value.clone());
        self.on_sender.send(value.clone()).ok();
        for (parent_id, key) in self.parents.read().unwrap().iter() { // rayon?
            if let Some(parent) = self.store.read().unwrap().get(parent_id) {
                parent.map_sender.send((key.clone(), value.clone())).ok();
                if let Some(parent_val) = parent.get_local_value_once() {
                    parent.on_sender.send(parent_val).ok();
                }
                *parent.value.write().unwrap() = None; // TODO update graph size (use helper method to change value?)
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
        let mut gun = Node::new();
        let _ = gun.get("Meneldor"); // Pick Tolkien names from https://www.behindthename.com/namesakes/list/tolkien/alpha
        assert!(gun.id.len() == 0);
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
        setup();
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
            node1.get("node1").put("Amandil".into());
            node2.get("node2").put("Beregond".into());
            let mut sub1 = node1.get("node2").on();
            let mut sub2 = node2.get("node1").on();
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
