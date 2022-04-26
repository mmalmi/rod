use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message},
    WebSocketStream
};
use tokio::sync::RwLock;
use url::Url;
use std::collections::HashMap;

use crate::types::{NetworkAdapter, GunMessage};
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

pub struct MemoryStorage {
    node: Node,
    graph_size_bytes: usize,
    store: Arc<RwLock<HashMap<String, NodeData>>>,
}

impl MemoryStorage {
    fn update_stats(&self) {
        let peer_id = self.node.get_peer_id();
        let mut stats = self.node.clone().get("node_stats").get(&peer_id);
        let store = self.store.clone();
        tokio::task::spawn(async move {
            loop {
                let count = store.read().unwrap().len().to_string();
                let graph_size_bytes = format!("{}B", size_format::SizeFormatterBinary::new(self.graph_size_bytes as u64).to_string());
                stats.get("graph_node_count").put(count.into());
                stats.get("graph_size_bytes").put(graph_size_bytes.into());
                sleep(Duration::from_millis(1000)).await;
            }
        });
    }
}

#[async_trait]
impl NetworkAdapter for MemoryStorage {
    fn new(node: Node) -> Self {
        MemoryStorage {
            node,
            graph_size_bytes: 0,
            store: HashMap::new(), // If we don't want to store everything in memory, this needs to use something like Redis or LevelDB. Or have a FileSystem adapter for persistence and evict the least important stuff from memory when it's full.
        }
    }

    async fn start(&self) {
        let my_id = "memory_storage".to_string();

        let mut rx = self.node.get_outgoing_msg_receiver();

        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await {
                    if message.from == my_id {
                        continue;
                    }

                    if let Some(put) = msg_obj.get("put") {
                        if let Some(put_obj) = put.as_object() {
                            self.incoming_put(&msg_str, &msg_id, &from, msg_obj, put_obj); // TODO move to mem or sled storage adapter
                        }
                    }
                    if let Some(get) = msg_obj.get("get") {
                        if let Some(get_obj) = get.as_object() {
                            self.seen_get_messages.write().unwrap().insert(msg_id.clone(), SeenGetMessage {
                                from: from.clone(),
                                last_reply_hash: "".to_string()
                            });
                            self.incoming_get(get_obj, from); // TODO move to mem or sled storage adapter
                            self.outgoing_message(&msg_str, from, msg_id, None); // TODO: randomly sample recipients
                        }
                    }
                    // handle get and put
                }
            }
        });
    }

    fn handle_get(&mut self, get: &serde_json::Map<String, SerdeJsonValue>, from: &String) {
        if let Some(id) = get.get("#") {
            if let Some(id) = id.as_str() {
                let data = self.store.read().unwrap().get(id).cloned();
                if let Some(data) = data {
                    debug!("have {}: {:?}", id, data);
                    {
                        let topic = id.split("/").next().unwrap_or("");
                        debug!("{} subscribed to {}", from, topic);
                        self.subscribers_by_topic.write().unwrap().entry(topic.to_string())
                            .or_insert_with(HashSet::new).insert(from.clone());
                    }
                    if let Some(key) = get.get(".") {
                        if let Some(key) = key.as_str() {
                            // todo: send data[key]
                            self.send_get_response(id.to_string(), data, from);
                        }
                    } else { // get all children of the (root level?) node
                        self.send_get_response(id.to_string(), data, from);
                    }



                } else {
                    debug!("have not {}", id);
                }
            }
        }
    }

    fn handle_put(&mut self, msg_str: &String, msg_id: &String, from: &String, msg_obj: &serde_json::Map<String, SerdeJsonValue>, put_obj: &serde_json::Map<String, SerdeJsonValue>) {
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
                        debug!("same reply already sent");
                        return;
                    } // failing these conditions, should we still send the ack to someone?
                    seen_get_message.last_reply_hash = content_hash;
                    recipients.insert(seen_get_message.from.clone());
                }
            }
        }
        for (updated_node_id, update_data) in put_obj.iter() {
            if !is_ack {
                let topic = updated_node_id.split("/").next().unwrap_or("");
                debug!("getting subscribers for topic {}", topic);
                if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
                    recipients.extend(subscribers.clone());
                }
            }
            if let Some(updated_at_times) = update_data["_"][">"].as_object() {
                for (child_key, incoming_val_updated_at) in updated_at_times.iter() {
                    if let Some(incoming_val_updated_at) = incoming_val_updated_at.as_f64() {
                        let update = || {
                            if let Some(new_value) = update_data.get(child_key) {
                                if let Ok(new_value) = serde_json::from_value::<GunValue>(new_value.clone()) {
                                    if let Some(subs) = self.subscriptions_by_node_id.read().unwrap().get(updated_node_id) {
                                        for sub in subs {
                                            sub.send(new_value.clone());
                                        }
                                    }
                                    debug!("saving new k-v {}: {:?}", updated_node_id, new_value);
                                    let mut write = self.store.write().unwrap();
                                    let data = write.entry(updated_node_id.to_string())
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
        }
        recipients.remove(from);
        self.outgoing_message(&msg_str, from, msg_id.clone(), Some(recipients));
    }
}


