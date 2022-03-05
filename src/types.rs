use serde::{Serialize, Deserialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use crate::Node;
use async_trait::async_trait;

/// Value types supported by gun.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum GunValue {
    Null,
    Bit(bool),
    Number(f64),
    Text(String),
    Link(usize),
    Children(BTreeMap<String, GunValue>),
}

impl GunValue {
    pub fn size(&self) -> usize {
        match self {
            GunValue::Text(s) => s.len(),
            _ => std::mem::size_of_val(self)
        }
    }
}

impl From<usize> for GunValue {
    fn from(n: usize) -> GunValue {
        GunValue::Number(n as f64)
    }
}

impl From<f32> for GunValue {
    fn from(n: f32) -> GunValue {
        GunValue::Number(n as f64)
    }
}

impl From<u64> for GunValue {
    fn from(n: u64) -> GunValue {
        GunValue::Number(n as f64)
    }
}

impl From<&str> for GunValue {
    fn from(s: &str) -> GunValue {
        GunValue::Text(s.to_string())
    }
}

impl From<String> for GunValue {
    fn from(s: String) -> GunValue {
        GunValue::Text(s)
    }
}

// This could actually be renamed to "Plugin" or "SyncAdapter"?
// After all, there's nothing networking-specific in this trait.
// Could be used for disk storage as well.
// -
// Adapters should probably use channels for communicating with the node, rather than calling Node::incoming_message?
// Faster if all incoming messages are handled by one task, instead of many tasks interrupting each other with RwLocks?
// -
// Can we get rid of async_trait?
/// Syncs the gun Node with other Nodes over various transports like websocket or multicast.
///
/// NetworkAdapters should communicate with the Node using [Node::get_outgoing_msg_receiver] and
/// [Node::incoming_message].
#[async_trait]
pub trait NetworkAdapter {
    fn new(node: Node) -> Self where Self: Sized;
    /// This is called on node.start_adapters()
    async fn start(&self);
}

/// Used internally to represent Gun network messages.
#[derive(Clone, Debug)]
pub struct GunMessage {
    pub msg: String,
    pub from: String,
    pub to: Option<HashSet<String>>
}

/// When full, every insert pushes out the oldest entry in the set.
///
/// Used to record last seen message IDs.
pub struct BoundedHashSet {
    set: HashSet<String>,
    queue: VecDeque<String>,
    max_entries: usize
}

impl BoundedHashSet {
    pub fn new(max_entries: usize) -> Self {
        BoundedHashSet {
            set: HashSet::new(),
            queue: VecDeque::new(),
            max_entries
        }
    }

    pub fn insert(&mut self, s: String) {
        if self.set.contains(&s) {
            return;
        }
        if self.queue.len() >= self.max_entries {
            if let Some(removed) = self.queue.pop_back() {
                self.set.remove(&removed);
            }
        }
        self.queue.push_front(s.clone());
        self.set.insert(s);
    }

    pub fn contains(&self, s: &str) -> bool {
        return self.set.contains(s);
    }
}

/// When full, every insert pushes out the oldest entry in the set.
///
/// Used to record last seen message IDs.
pub struct BoundedHashMap<K, V> {
    map: HashMap<K, V>,
    queue: VecDeque<K>,
    max_entries: usize
}

impl<K: Clone + std::hash::Hash + std::cmp::Eq, V> BoundedHashMap<K, V> {
    pub fn new(max_entries: usize) -> Self {
        BoundedHashMap {
            map: HashMap::new(),
            queue: VecDeque::new(),
            max_entries
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.queue.len() >= self.max_entries {
            if let Some(removed) = self.queue.pop_back() {
                self.map.remove(&removed);
            }
        }
        if !self.map.contains_key(&key) {
            self.queue.push_front(key.clone());
        }
        self.map.insert(key, value);
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.map.get_mut(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        return self.map.contains_key(key);
    }
}

// Nodes need to be cloneable so that each instance points to the same data in the graph.
// But can we somehow wrap Node itself into Arc<RwLock<>> instead of wrapping all its properties?
// Arc<RwLock<NodeInner>> pattern?
// The code is not pretty with all these Arc-RwLock read/write().unwraps().
pub(crate) type Value = Arc<RwLock<Option<GunValue>>>;
pub(crate) type Children = Arc<RwLock<BTreeMap<String, usize>>>;
pub(crate) type Parents = Arc<RwLock<HashSet<(usize, String)>>>;
pub(crate) type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;
pub(crate) type NetworkAdapters = Arc<RwLock<HashMap<String, Box<dyn NetworkAdapter + Send + Sync>>>>;