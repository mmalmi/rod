use serde::{Serialize, Deserialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock};
use crate::Node;
use async_trait::async_trait;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum GunValue {
    Null,
    Bit(bool),
    Number(f32),
    Text(String),
    Link(usize),
    Children(BTreeMap<String, GunValue>)
}

impl From<usize> for GunValue {
    fn from(n: usize) -> GunValue {
        GunValue::Number(n as f32)
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
    fn send_str(&self, m: &String);
}

// Nodes need to be cloneable so that each instance points to the same data in the graph.
// But can we somehow wrap Node itself into Arc<RwLock<>> instead of wrapping all its properties?
// The code is not pretty with all these Arc-RwLock read/write().unwraps().
pub type Callback = Box<dyn (Fn(GunValue, String) -> ()) + Send + Sync>;
pub type Value = Arc<RwLock<Option<GunValue>>>;
pub type Children = Arc<RwLock<BTreeMap<String, usize>>>;
pub type Parents = Arc<RwLock<HashSet<(usize, String)>>>;
pub type Subscriptions = Arc<RwLock<HashMap<usize, Callback>>>;
pub type SharedNodeStore = Arc<RwLock<HashMap<usize, Node>>>;
pub type SeenMessages = Arc<RwLock<HashSet<String>>>;
pub type NetworkAdapters = Arc<RwLock<HashMap<String, Box<dyn NetworkAdapter + Send + Sync>>>>;