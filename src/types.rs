use serde::{Serialize, Deserialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use crate::Node;
use async_trait::async_trait;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum GunValue {
    Null,
    Bit(bool),
    Number(f64),
    Text(String),
    Link(usize),
    Children(BTreeMap<String, GunValue>)
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

#[async_trait]
pub trait NetworkAdapter {
    fn new(node: Node) -> Self where Self: Sized;
    async fn start(&self);
    fn stop(&self);
}

#[derive(Clone)]
pub struct GunMessage {
    pub msg: String,
    pub from: String
}

pub struct BoundedHashSet {
    set: HashSet<String>,
    queue: VecDeque<String>,
    max_size: usize
}

impl BoundedHashSet {
    pub fn new(max_size: usize) -> Self {
        BoundedHashSet {
            set: HashSet::new(),
            queue: VecDeque::new(),
            max_size
        }
    }

    pub fn insert(&mut self, s: String) {
        if self.set.contains(&s) {
            return;
        }
        if self.queue.len() >= self.max_size {
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

// Nodes need to be cloneable so that each instance points to the same data in the graph.
// But can we somehow wrap Node itself into Arc<RwLock<>> instead of wrapping all its properties?
// Arc<RwLock<NodeInner>> pattern?
// The code is not pretty with all these Arc-RwLock read/write().unwraps().
pub type Callback = Box<dyn (Fn(GunValue, String) -> ()) + Send + Sync>;
pub type Value = Arc<RwLock<Option<GunValue>>>;
pub type Children = Arc<RwLock<BTreeMap<String, usize>>>;
pub type Parents = Arc<RwLock<HashSet<(usize, String)>>>;
pub type Subscriptions = Arc<RwLock<HashMap<usize, Callback>>>;
pub type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;
pub type NetworkAdapters = Arc<RwLock<HashMap<String, Box<dyn NetworkAdapter + Send + Sync>>>>;