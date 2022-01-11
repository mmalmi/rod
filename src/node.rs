use std::collections::{BTreeMap, HashSet};
use std::time::{SystemTime, Instant};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
    RwLock
};
use std::env;
use serde_json::{json, Value as SerdeJsonValue};
use crate::types::*;
use crate::utils::random_string;
use crate::adapters::WebsocketServer;
use crate::adapters::WebsocketClient;
use crate::adapters::Multicast;
use log::{debug};
use tokio::time::{sleep, Duration};
use tokio::sync::broadcast;
use sysinfo::{NetworkExt, NetworksExt, ProcessExt, ProcessorExt, System, SystemExt};

static SEEN_MSGS_MAX_SIZE: usize = 10000;
static COUNTER: AtomicUsize = AtomicUsize::new(1);
fn get_id() -> usize { COUNTER.fetch_add(1, Ordering::Relaxed) }

// TODO extract networking to struct Mesh
// TODO proper automatic tests
// TODO persist data by saving root node to indexedDB as serialized by serde?
// Node { node: Arc<RwLock<NodeInner>> } instead of Arc<RwLock> for each member? compare performance
// TODO connections don't seem to be closed / timeouted properly when client has disconnected

#[derive(Clone)]
pub struct Node {
    id: usize,
    updated_at: Arc<RwLock<f64>>, // TODO: Option<f64>?
    key: String,
    path: Vec<String>,
    value: Value,
    children: Children,
    parents: Parents,
    on_subscriptions: Subscriptions,
    map_subscriptions: Subscriptions,
    store: SharedNodeStore,
    network_adapters: NetworkAdapters,
    seen_messages: Arc<RwLock<BoundedHashSet>>,
    peer_id: Arc<RwLock<String>>,
    msg_counter: Arc<AtomicUsize>,
    outgoing_msg_sender: broadcast::Sender<GunMessage>,
}

impl Node {
    pub fn new() -> Self {
        let channel_size: usize = match env::var("RUST_CHANNEL_SIZE") {
            Ok(p) => p.parse::<usize>().unwrap(),
            _ => 10
        };
        let (sender, _receiver) = broadcast::channel::<GunMessage>(channel_size);
        let node = Self {
            id: 0,
            updated_at: Arc::new(RwLock::new(0.0)),
            key: "".to_string(),
            path: Vec::new(),
            value: Value::default(),
            children: Children::default(),
            parents: Parents::default(),
            on_subscriptions: Subscriptions::default(),
            map_subscriptions: Subscriptions::default(),
            store: SharedNodeStore::default(),
            network_adapters: NetworkAdapters::default(),
            seen_messages: Arc::new(RwLock::new(BoundedHashSet::new(SEEN_MSGS_MAX_SIZE))),
            peer_id: Arc::new(RwLock::new(random_string(16))),
            msg_counter: Arc::new(AtomicUsize::new(0)),
            outgoing_msg_sender: sender,
        };
        //let _multicast = Multicast::new(node.clone());
        let server = WebsocketServer::new(node.clone());
        let client = WebsocketClient::new(node.clone());
        //node.network_adapters.write().unwrap().insert("multicast".to_string(), Box::new(multicast));
        node.network_adapters.write().unwrap().insert("ws_server".to_string(), Box::new(server));
        node.network_adapters.write().unwrap().insert("ws_client".to_string(), Box::new(client));
        node
    }

    pub fn get_outgoing_msg_receiver(&self) -> broadcast::Receiver<GunMessage> {
        self.outgoing_msg_sender.subscribe()
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
                let mut stats = node.get("node_stats").get(&peer_id);
                stats.get("msgs_per_second").put(node.msg_counter.load(Ordering::Relaxed).into());
                node.msg_counter.store(0, Ordering::Relaxed);
                stats.get("graph_node_count").put(count.into());
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

    pub async fn start(&mut self) {
        let adapters = self.network_adapters.read().unwrap();
        let mut futures = Vec::new();
        for adapter in adapters.values() {
            futures.push(adapter.start()); // adapters must be non-blocking: use async functions or spawn_blocking
        }
        self.update_stats();
        futures::future::join_all(futures).await;
    }

    fn new_child(&self, key: String) -> usize {
        assert!(key.len() > 0, "Key length must be greater than zero");
        debug!("new child {} {}", self.path.join("/"), key);
        let mut parents = HashSet::new();
        parents.insert((self.id, key.clone()));
        let mut path = self.path.clone();
        if self.key.len() > 0 {
            path.push(self.key.clone());
        }
        let id = get_id();
        let node = Self {
            id,
            updated_at: Arc::new(RwLock::new(0.0)),
            key: key.clone(),
            path,
            value: Value::default(),
            children: Children::default(),
            parents: Arc::new(RwLock::new(parents)),
            on_subscriptions: Subscriptions::default(),
            map_subscriptions: Subscriptions::default(),
            store: self.store.clone(),
            network_adapters: self.network_adapters.clone(),
            seen_messages: Arc::new(RwLock::new(BoundedHashSet::new(SEEN_MSGS_MAX_SIZE))),
            peer_id: self.peer_id.clone(),
            msg_counter: self.msg_counter.clone(),
            outgoing_msg_sender: self.outgoing_msg_sender.clone(),
        };
        self.store.write().unwrap().insert(id, node);
        self.children.write().unwrap().insert(key, id);
        id
    }

    pub fn off(&mut self, subscription_id: usize) {
        self.on_subscriptions.write().unwrap().remove(&subscription_id);
        self.map_subscriptions.write().unwrap().remove(&subscription_id);
    }

    pub fn get_peer_id(&self) -> String {
        self.peer_id.read().unwrap().to_string()
    }

    pub fn on(&mut self, callback: Callback) -> usize {
        self.once_local(&callback, &self.key.clone());
        let subscription_id = get_id();
        self.on_subscriptions.write().unwrap().insert(subscription_id, callback);
        if self.network_adapters.read().unwrap().len() > 0 {
            let (m, id) = self.create_get_msg();
            self.send_to_adapters(&m.to_string(), &self.get_peer_id(), id);
        }
        subscription_id
    }

    pub fn get(&mut self, key: &str) -> Node {
        if key == "" {
            return self.clone();
        }
        let id = self.get_child_id(key.to_string());
        let mut node = self.store.read().unwrap().get(&id).unwrap().clone();
        node.key = key.to_string();
        node
    }

    pub fn map(&self, callback: Callback) -> usize {
        for (key, child_id) in self.children.read().unwrap().iter() { // TODO can be faster with rayon multithreading?
            if let Some(child) = self.store.read().unwrap().get(&child_id) {
                child.clone().once_local(&callback, key);
            }
        }
        let subscription_id = get_id();
        self.map_subscriptions.write().unwrap().insert(subscription_id, callback);
        // TODO: send get messages to adapters
        subscription_id
    }

    fn get_child_id(&mut self, key: String) -> usize {
        let existing_id = match self.children.read().unwrap().get(&key) {
            Some(node_id) => Some(*node_id),
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
                full_path: {
                    "_": {
                        "#": full_path,
                        ">": {
                            key: updated_at
                        }
                    },
                    key: value
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
                    "#": path,
                    ">": {
                        node_name: updated_at
                    }
                },
                node_name: {
                    "#": self.path[..(i+1)].join("/")
                }
            });
            puts[path] = path_obj;
        }
        (json.to_string(), msg_id)
    }

    fn incoming_message_json(&mut self, msg: &SerdeJsonValue, is_from_array: bool, msg_str: Option<String>, from: &String) {
        if let Some(array) = msg.as_array() {
            if is_from_array { return; } // don't allow array inside array
            for msg in array.iter() {
                self.incoming_message_json(msg, true, None, from);
            }
            return;
        }
        if let Some(obj) = msg.as_object() {
            if let Some(msg_id) = obj.get("#") {
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

                    if let Some(put) = obj.get("put") {
                        if let Some(obj) = put.as_object() {
                            self.incoming_put(obj);
                            self.send_to_adapters(&msg_str, from, msg_id.clone());
                        }
                    }
                    if let Some(get) = obj.get("get") {
                        if let Some(obj) = get.as_object() {
                            self.incoming_get(obj);
                            self.send_to_adapters(&msg_str, from, msg_id);
                        }
                    }
                }
            } else {
                debug!("msg without id: {}\n", msg);
            }
        }
    }

    pub fn incoming_message(&mut self, msg: String, from: &String) {
        self.msg_counter.fetch_add(1, Ordering::Relaxed);
        let json: SerdeJsonValue = match serde_json::from_str(&msg) {
            Ok(json) => json,
            Err(_) => { return; }
        };
        self.incoming_message_json(&json, false, Some(msg), from);
    }

    fn incoming_put(&mut self, put: &serde_json::Map<String, SerdeJsonValue>) {
        for (updated_key, update_data) in put.iter() {
            let mut node = self.get(updated_key);
            for node_name in updated_key.split("/").nth(1) {
                node = node.get(node_name);
            }
            if let Some(updated_at_times) = update_data["_"][">"].as_object() {
                for (child_key, incoming_val_updated_at) in updated_at_times.iter() {
                    if let Some(incoming_val_updated_at) = incoming_val_updated_at.as_f64() {
                        let mut child = node.get(child_key);
                        if *child.updated_at.read().unwrap() < incoming_val_updated_at {
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
    }

    fn _children_to_gun_value(&self, children: &BTreeMap<String, usize>) -> GunValue {
        let mut map = BTreeMap::<String, GunValue>::new();
        for (key, child_id) in children.iter() { // TODO faster with rayon?
            let child_value: Option<GunValue> = match self.store.read().unwrap().get(&child_id) {
                Some(child) => match &*(child.value.read().unwrap()) {
                    Some(value) => Some(value.clone()),
                    _ => None
                },
                _ => None
            };
            if let Some(value) = child_value {
                map.insert(key.clone(), value);
            } else { // return child Node object
                map.insert(key.clone(), GunValue::Link(*child_id));
            }
        }
        GunValue::Children(map)
    }

    pub fn once_local(&mut self, callback: &Callback, key: &String) {
        if let Some(value) = self.get_gun_value() {
            callback(value, key.clone());
        }
    }

    fn send_to_adapters(&self, msg: &String, from: &String, msg_id: String) {
        debug!("sending msg {}", &msg_id);
        self.seen_messages.write().unwrap().insert(msg_id); // TODO: doesn't seem to work, at least on multicast
        self.outgoing_msg_sender.send(GunMessage { msg: msg.clone(), from: from.clone() });
    }

    fn get_gun_value(&self) -> Option<GunValue> {
        let value = self.value.read().unwrap();
        if value.is_some() {
            value.clone()
        } else {
            let children = self.children.read().unwrap();
            if !children.is_empty() {
                let obj = self._children_to_gun_value(&children);
                return Some(obj)
            }
            None
        }
    }

    fn send_get_response_if_have(&self) {
        if let Some(value) = self.get_gun_value() {
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
            self.send_to_adapters(&json, &self.get_peer_id(), msg_id);
        }
    }

    fn incoming_get(&mut self, get: &serde_json::Map<String, SerdeJsonValue>) {
        if let Some(path) = get.get("#") {
            if let Some(path) = path.as_str() {
                if let Some(key) = get.get(".") {
                    // debug!("yes . {} {}", path, key);
                    if let Some(key) = key.as_str() {
                        let mut split = path.split("/");
                        let mut node = self.get(split.nth(0).unwrap());
                        for node_name in split.nth(0) {
                            node = node.get(node_name); // TODO get only existing nodes in order to not spam our graph with empties
                        }
                        node = node.get(key);
                        node.send_get_response_if_have(); // TODO don't send response to everyone
                    }
                } else {
                    let mut split = path.split("/");
                    let mut node = self.get(split.nth(0).unwrap());
                    for node_name in split.nth(0) {
                        node = node.get(node_name); // TODO get only existing nodes in order to not spam our graph with empties
                    }
                    node.send_get_response_if_have();

                    // debug!("no {}", path);
                }
            }
        }
    }

    pub fn put(&mut self, value: GunValue) {
        let time: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as f64;
        self.put_local(value.clone(), time);
        if self.network_adapters.read().unwrap().len() > 0 {
            let (m, id) = self.create_put_msg(&value, time);
            self.send_to_adapters(&m, &self.get_peer_id(), id);
        }
    }

    fn put_local(&mut self, value: GunValue, time: f64) {
        //debug!("put_local\n {}\n {:?}\n", self.path.join("/"), value);
        // root.get(soul).get(key).put(jsvalue)
        // TODO handle javascript Object values
        // TODO: if "children" is replaced with "value", remove backreference from linked objects
        *self.updated_at.write().unwrap() = time;
        *self.value.write().unwrap() = Some(value.clone());
        *self.children.write().unwrap() = BTreeMap::new();
        for callback in self.on_subscriptions.read().unwrap().values() { // rayon?
            callback(value.clone(), self.key.clone());
        }
        for (parent_id, key) in self.parents.read().unwrap().iter() { // rayon?
            if let Some(parent) = self.store.read().unwrap().get(parent_id) {
                let mut parent_clone = parent.clone();
                for callback in parent.clone().map_subscriptions.read().unwrap().values() {
                    callback(value.clone(), key.clone());
                }
                for callback in parent.on_subscriptions.read().unwrap().values() {
                    parent_clone.once_local(&callback, key);
                }
                *parent.value.write().unwrap() = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Node;
    use crate::types::GunValue;
    use std::cell::RefCell;
    use std::time::{Duration, Instant};

    // TODO proper test
    // TODO benchmark
    #[test]
    fn it_doesnt_error() {
        let mut gun = Node::new();
        let _ = gun.get("Meneldor"); // Pick Tolkien names from https://www.behindthename.com/namesakes/list/tolkien/alpha
        assert_eq!(gun.id, 0);
    }

    #[test]
    fn put_and_get() {
        let mut gun = Node::new();
        let mut node = gun.get("Finglas");
        node.put("Fingolfin".into());
        node.on(Box::new(|value: GunValue, key: String| { // TODO how to do it without Box? https://stackoverflow.com/questions/41081240/idiomatic-callbacks-in-rust
            assert!(matches!(value, GunValue::Text(_)));
            if let GunValue::Text(str) = value {
                assert_eq!(&str, "Fingolfin");
            }
        }));
    }

    //var i = 28000, j = i, s = +new Date; while(--i){ gun.get('a'+i).get('lol').put(i+'yo') } console.log(j / ((+new Date - s) / 1000), 'ops/sec');

    #[test]
    fn write_benchmark() { // to see the result with optimized binary, run: cargo test --release -- --nocapture
        let start = Instant::now();
        let mut gun = Node::new();
        let n = 100000;
        for i in 0..n {
            gun.get(&format!("a{:?}", i)).get("Pelendur").put(format!("{:?}b", i).into());
        }
        let duration = start.elapsed();
        let per_second = (n as f64) / (duration.as_nanos() as f64) * 1000000000.0;
        debug!("Wrote {} entries in {:?} ({} / second)", n, duration, per_second);
    }
}
