use std::collections::{HashMap, HashSet, BTreeMap};

use crate::message::{Message, Put, Get};
use crate::types::NetworkAdapter;
use crate::Node;
use crate::types::*;

use async_trait::async_trait;
use log::{debug};
use std::sync::{Arc, RwLock};
use tokio::time::{sleep, Duration};

pub struct MemoryStorage {
    id: String,
    node: Node,
    graph_size_bytes: Arc<RwLock<usize>>,
    store: Arc<RwLock<HashMap<String, NodeData>>>,
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

    fn handle_get(msg: Get, my_id: String, store: Arc<RwLock<HashMap<String, NodeData>>>, node: Node) {
        if msg.from == my_id {
            return;
        }

        let data = store.read().unwrap().get(&msg.node_id).cloned();
        if let Some(data) = data {
            debug!("have {}: {:?}", msg.id, data);
            match data.value {
                GunValue::Children(map) => {
                    let mut recipients = HashSet::new();
                    recipients.insert(msg.from.clone());
                    let put = Put::new(map.clone(), Some(msg.id.clone()));
                    node.get_incoming_msg_sender().send(Message::Put(put));
                },
                _ => {}
            };
        } else {
            debug!("have not {}", msg.id);
        }
    }

    fn handle_put(msg: Put, my_id: String, store: Arc<RwLock<HashMap<String, NodeData>>>) {
        if msg.from == my_id {
            return;
        }

        for (node_id, update_data) in msg.updated_nodes.iter() {
            let update = || {
                debug!("saving new k-v {}: {:?}", node_id, update_data);
                let mut write = store.write().unwrap();
                let data = write.entry(node_id.to_string())
                    .or_insert_with(NodeData::default);
                let mut map = match &data.value {
                    GunValue::Children(map) => map.clone(),
                    _ => BTreeMap::<String, NodeData>::new()
                };
                map.insert(node_id.clone(), update_data.clone());
                data.value = GunValue::Children(map);
                data.updated_at = update_data.updated_at;
            };

            let existing_data: Option<NodeData>;
            { // to prevent store deadlock
                existing_data = store.read().unwrap().get(node_id).cloned();
            }
            if let Some(existing_data) = existing_data {
                // TODO: merge
                if existing_data.updated_at <= update_data.updated_at {
                    update();
                }
            } else {
                update();
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


