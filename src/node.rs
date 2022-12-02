use crate::actor::{Actor, ActorContext, Addr};
use crate::message::{Get, Message, Put};
use crate::router::Router;
use crate::types::{Children, NodeData, Value};
use crate::utils::random_string;
use crate::adapters::MemoryStorage;
use async_trait::async_trait;
use log::{debug, info};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::SystemTime; // TODO get time from ActorContext
use tokio::sync::broadcast; // TODO replace with generics: Sender and Receiver traits?

static BROADCAST_CHANNEL_SIZE: usize = 10;

// TODO proper automatic tests
// Node { node: Arc<RwLock<NodeInner>> } instead of Arc<RwLock> for each member? compare performance
// TODO connections don't seem to be closed / timeouted properly when client has disconnected
// TODO should use async RwLock everywhere?

// TODO: separate configs for each adapter?
/// [Node] configuration object.
#[derive(Clone)]
pub struct Config {
    pub allow_public_space: bool,
    /// Prioritize data storage for this public key. Format: x.y where x and y are base64 encoded ECDSA public key coordinates.
    /// Example: hyECQHwSo7fgr2MVfPyakvayPeixxsaAWVtZ-vbaiSc.TXIp8MnCtrnW6n2MrYquWPcc-DTmZzMBmc2yaGv9gIU
    pub my_pub: Option<String>,
    /// Show node stats at /stats?
    pub stats: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            allow_public_space: true,
            stats: true,
            my_pub: None,
        }
    }
}

/// A Graph Node that provides an API for graph traversal.
/// Sends, processes and relays Put & Get messages between storage and transport adapters.
#[derive(Clone)]
pub struct Node {
    uid: Arc<RwLock<String>>,
    path: Vec<String>,
    children: Arc<RwLock<BTreeMap<String, Node>>>,
    parent: Arc<RwLock<Option<(String, Node)>>>,
    on_sender: broadcast::Sender<Value>,
    map_sender: broadcast::Sender<(String, Value)>,
    actor_context: Box<ActorContext>,
    addr: Arc<RwLock<Option<Addr>>>,
    router: Arc<RwLock<Option<Addr>>>,
}

#[async_trait]
impl Actor for Node {
    async fn handle(&mut self, msg: Message, _context: &ActorContext) {
        match msg {
            Message::Put(put) => self.handle_put(put),
            _ => {}
        }
    }
}

impl Node {
    /// Create a new root-level Node using default configuration. No network or storage adapters are started.
    pub fn new() -> Self {
        // Use MemoryStorage by default
        let storage = MemoryStorage::new();
        Self::new_with_config(Config::default(), vec![Box::new(storage)], Vec::new())
    }

    pub fn id(&self) -> String {
        self.uid.read().unwrap().clone()
    }

    pub fn peer_id(&self) -> String {
        self.actor_context.peer_id.read().unwrap().clone()
    }

    /// Create a new root-level Node using custom configuration. Starts the default or configured network and storage adapters.
    ///
    /// # Examples
    ///
    /// ```
    /// tokio_test::block_on(async {
    ///
    ///     use rod::{Node, Config, Value};
    ///     use rod::adapters::{MemoryStorage, OutgoingWebsocketManager};
    ///
    ///     let config = Config::default();
    ///     let memory_storage = Box::new(MemoryStorage::new());
    ///     let ws_client = Box::new(OutgoingWebsocketManager::new(config.clone(), vec!["wss://some-rod-server.com/ws".to_string()]));
    ///     let mut db = Node::new_with_config(config.clone(), vec![memory_storage], vec![ws_client]);
    ///     let mut sub = db.get("greeting").on();
    ///     db.get("greeting").put("Hello World!".into());
    ///     if let Value::Text(str) = sub.recv().await.unwrap() {
    ///         assert_eq!(&str, "Hello World!");
    ///     }
    ///
    /// })
    /// ```
    pub fn new_with_config(
        config: Config,
        storage_adapters: Vec<Box<dyn Actor>>,
        network_adapters: Vec<Box<dyn Actor>>,
    ) -> Self {
        let actor_context = ActorContext::new(random_string(16));
        let mut node = Self {
            path: vec![],
            uid: Arc::new(RwLock::new("".to_string())),
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parent: Arc::new(RwLock::new(None)),
            on_sender: broadcast::channel::<Value>(BROADCAST_CHANNEL_SIZE).0,
            map_sender: broadcast::channel::<(String, Value)>(BROADCAST_CHANNEL_SIZE).0,
            addr: Arc::new(RwLock::new(None)),
            router: Arc::new(RwLock::new(None)),
            actor_context: Box::new(actor_context),
        };

        node.actor_context.node = Some(node.clone());
        let addr = node.actor_context.start_actor(Box::new(node.clone()));
        *node.addr.write().unwrap() = Some(addr);

        let router = Box::new(Router::new(config, storage_adapters, network_adapters)); // actually, we should communicate with
                                                                                        // MemoryStorage (or sled), which has a special role in maintaining our version of the current state?
                                                                                        // MemoryStorage can then communicate with router as needed.
        let router_addr = node.actor_context.start_router(router);
        *node.router.write().unwrap() = Some(router_addr);

        node
    }

    fn handle_put(&mut self, put: Put) {
        // TODO accept puts only from our memory/sled adapter, which is supposed to serve the latest version.
        // Or store latest NodeData in Node? Would eat up memory though.
        for (node_id, node_data) in put.updated_nodes {
            if node_id == *self.uid.read().unwrap() {
                for (child, child_data) in node_data {
                    if let Some(child) = self.children.read().unwrap().get(&child) {
                        let _ = child.on_sender.send(child_data.value.clone());
                    }
                    let _ = self
                        .map_sender
                        .send((child.to_string(), child_data.value.clone()));
                }
            }
        }
    }

    fn new_child(&self, key: String) -> Node {
        assert!(key.len() > 0, "Key length must be greater than zero");
        debug!("new child {}", key);
        let mut path = self.path.clone();
        path.push(key.clone());
        let new_child_uid = path.join("/");
        debug!("new_child_uid {}", new_child_uid);
        let node = Self {
            path,
            children: Arc::new(RwLock::new(BTreeMap::new())),
            parent: Arc::new(RwLock::new(Some((
                self.uid.read().unwrap().clone(),
                self.clone(),
            )))),
            on_sender: broadcast::channel::<Value>(BROADCAST_CHANNEL_SIZE).0,
            map_sender: broadcast::channel::<(String, Value)>(BROADCAST_CHANNEL_SIZE).0,
            uid: Arc::new(RwLock::new(new_child_uid)),
            router: self.router.clone(),
            addr: Arc::new(RwLock::new(None)),
            actor_context: self.actor_context.clone(),
        };
        let addr = self.actor_context.start_actor(Box::new(node.clone()));
        *node.addr.write().unwrap() = Some(addr);
        self.children.write().unwrap().insert(key, node.clone());
        node
    }

    /// Subscribe to the Node's value.
    pub fn on(&mut self) -> broadcast::Receiver<Value> {
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
            addr = parent.addr.read().unwrap().clone().unwrap();
        } else {
            node_id = self.uid.read().unwrap().to_string();
            addr = self.addr.read().unwrap().clone().unwrap();
        }
        let get = Get::new(node_id, key, addr);
        if let Some(router) = self.router.read().unwrap().clone() {
            let _ = router.send(Message::Get(get));
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
    pub fn map(&self) -> broadcast::Receiver<(String, Value)> {
        // TODO: send get messages to adapters!!
        self.map_sender.subscribe()
    }

    fn add_parent_nodes(
        &mut self,
        updated_nodes: &mut BTreeMap<String, Children>,
        value: Value,
        updated_at: f64,
    ) {
        let parent = &*self.parent.read().unwrap();
        if let Some((parent_id, parent)) = parent {
            if parent_id == "" {
                return; // TODO: this breaks first_put_then_get test
            }
            let mut parent = parent.clone();
            let mut children = Children::default();
            children.insert(
                self.path.last().unwrap().clone(),
                NodeData {
                    value: value.clone(),
                    updated_at,
                },
            );
            updated_nodes.insert(parent_id.to_string(), children);
            parent.add_parent_nodes(updated_nodes, Value::Link(parent.id()), updated_at);
        }
    }

    /// Set a Value for the Node.
    pub fn put(&mut self, value: Value) {
        let updated_at: f64 = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64;
        self.on_sender.send(value.clone()).ok();
        debug!("put {}", value.to_string());
        let mut updated_nodes = BTreeMap::new();
        self.add_parent_nodes(&mut updated_nodes, value, updated_at);
        let my_addr = self.addr.read().unwrap().clone().unwrap();
        let put = Put::new(updated_nodes, None, my_addr);
        if let Some(router) = &*self.router.read().unwrap() {
            let _ = router.send(Message::Put(put));
        }
    }

    pub fn stop(&mut self) {
        info!("Node stopping");
        self.actor_context.stop();
    }
}
