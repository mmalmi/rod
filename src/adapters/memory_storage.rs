use std::collections::{HashMap, HashSet, BTreeMap};

use crate::message::{Message, Put, Get};
use crate::types::NetworkAdapter;
use crate::Node;
use crate::types::*;

use async_trait::async_trait;
use log::{debug, error};
use std::sync::{Arc, RwLock};
use tokio::time::{sleep, Duration};

pub struct MemoryStorage {
    id: String,
    node: Node,
    graph_size_bytes: Arc<RwLock<usize>>,
    store: Arc<RwLock<HashMap<String, Children>>>,
}

impl MemoryStorage {
    fn update_stats(&self) {
        let peer_id = self.node.get_peer_id();
        let mut stats = self.node.clone().get("node_stats").get(&peer_id);
        let store = self.store.clone();
        let graph_size_bytes = self.graph_size_bytes.clone();
        tokio::task::spawn(async move {
            loop {
                let count = store.read().unwrap().len().to_string();
                let size = *graph_size_bytes.read().unwrap();
                let size = format!("{}B", size_format::SizeFormatterBinary::new(size as u64).to_string());
                stats.get("graph_node_count").put(count.into());
                stats.get("graph_size_bytes").put(size.into());
                sleep(Duration::from_millis(1000)).await;
            }
        });
    }

    fn handle_get(msg: Get, my_id: String, store: Arc<RwLock<HashMap<String, Children>>>, node: Node) {
        if msg.from == my_id {
            return;
        }

        if let Some(children) = store.read().unwrap().get(&msg.node_id).cloned() {
            debug!("have {}: {:?}", msg.node_id, children);
            let reply_with_children = match msg.child_key {
                Some(child_key) => { // reply with specific child if it's found
                    match children.get(&child_key) {
                        Some(child_val) => {
                            let mut r = BTreeMap::new();
                            r.insert(child_key.clone(), child_val.clone());
                            r
                        },
                        None => { return; }
                    }
                },
                None => children.clone() // reply with all children of this node
            };
            let mut reply_with_nodes = BTreeMap::new();
            reply_with_nodes.insert(msg.node_id.clone(), reply_with_children);
            let mut recipients = HashSet::new();
            recipients.insert(msg.from.clone());
            let put = Put::new(reply_with_nodes, Some(msg.id.clone()));
            if let Err(e) = node.get_incoming_msg_sender().try_send(Message::Put(put)) {
                error!("failed to send incoming message to node: {}", e);
            }
        } else {
            debug!("have not {}", msg.node_id);
        }
    }

    fn handle_put(msg: Put, my_id: String, store: Arc<RwLock<HashMap<String, Children>>>) {
        if msg.from == my_id {
            return;
        }

        for (node_id, update_data) in msg.updated_nodes.iter() {
            debug!("saving k-v {}: {:?}", node_id, update_data);
            let mut write = store.write().unwrap();
            if let Some(children) = write.get_mut(node_id) {
                for (child_id, child_data) in update_data {
                    if let Some(existing) = children.get(child_id) {
                        if child_data.updated_at >= existing.updated_at {
                            children.insert(child_id.clone(), child_data.clone());
                        }
                    } else {
                        children.insert(child_id.clone(), child_data.clone());
                    }
                }
            } else {
                write.insert(node_id.to_string(), update_data.clone());
            }
        }
    }
}

#[async_trait]
impl NetworkAdapter for MemoryStorage {
    fn new(node: Node) -> Self {
        MemoryStorage {
            id: "memory_storage".to_string(),
            node,
            graph_size_bytes: Arc::new(RwLock::new(0)),
            store: Arc::new(RwLock::new(HashMap::new())), // If we don't want to store everything in memory, this needs to use something like Redis or LevelDB. Or have a FileSystem adapter for persistence and evict the least important stuff from memory when it's full.
        }
    }

    async fn start(&self) {
        let store = self.store.clone();
        let my_id = self.id.clone();
        let node = self.node.clone();

        if node.config.read().unwrap().stats {
            self.update_stats();
        }

        let mut rx = node.get_outgoing_msg_receiver();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await {
                    match message {
                        Message::Get(get) => Self::handle_get(get.clone(), my_id.clone(), store.clone(), node.clone()),
                        Message::Put(put) => Self::handle_put(put.clone(), my_id.clone(), store.clone()),
                        _ => {}
                    }
                }
            }
        });
    }
}


