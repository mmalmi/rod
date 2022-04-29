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

    fn handle_get(&mut self, msg: Get) {
        if msg.from == self.id {
            return;
        }

        let data = self.store.read().unwrap().get(&msg.node_id).cloned();
        if let Some(data) = data {
            debug!("have {}: {:?}", msg.id, data);
            match data.value {
                GunValue::Children(map) => {
                    let mut recipients = HashSet::new();
                    recipients.insert(msg.from.clone());
                    let put = Put::new(map.clone(), Some(msg.id.clone()));
                    self.node.get_incoming_msg_sender().send(Message::Put(put));
                },
                _ => {}
            };
        } else {
            debug!("have not {}", msg.id);
        }
    }

    fn handle_put(&mut self, msg: Put) {
        if msg.from == self.id {
            return;
        }

        for (node_id, update_data) in msg.updated_nodes.iter() {
            let update = || {
                if let Some(new_value) = update_data.get(child_key) {
                    if let Ok(new_value) = serde_json::from_value::<GunValue>(new_value.clone()) {
                        /*
                        if let Some(subs) = self.subscriptions_by_node_id.read().unwrap().get(node_id) {
                            for sub in subs {
                                sub.send(new_value.clone());
                            }
                        }
                        */
                        debug!("saving new k-v {}: {:?}", node_id, new_value);
                        let mut write = self.store.write().unwrap();
                        let data = write.entry(node_id.to_string())
                            .or_insert_with(NodeData::default);
                        let mut map = match &data.value {
                            GunValue::Children(map) => map.clone(),
                            _ => BTreeMap::<String, GunValue>::new()
                        };
                        map.insert(child_key.clone(), new_value);
                        data.value = GunValue::Children(map);
                        data.updated_at = incoming_val_updated_at;
                    }
                }
            };

            let existing_data: Option<NodeData>;
            { // to prevent store deadlock
                existing_data = self.store.read().unwrap().get(child_key).cloned();
            }
            if let Some(existing_data) = existing_data {
                // TODO: merge
                if existing_data.updated_at <= incoming_val_updated_at {
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
        self.update_stats();

        let mut rx = self.node.get_outgoing_msg_receiver();

        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await {
                    match message {
                        Message::Get(get) => self.handle_get(get),
                        Message::Put(put) => self.handle_put(put),
                        _ => {}
                    }
                }
            }
        });
    }
}


