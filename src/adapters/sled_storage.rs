use std::collections::{HashMap, HashSet, BTreeMap};

use crate::message::{Message, Put, Get};
use crate::types::NetworkAdapter;
use crate::Node;
use crate::types::*;

use async_trait::async_trait;
use log::{debug, error};
use std::sync::{Arc, RwLock};
use sled;

use crate::adapters::websocket_server::OutgoingMessage;

macro_rules! unwrap_or_return {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                error!("{}", e);
                return;
            },
        }
    }
}

pub struct SledStorage {
    id: String,
    node: Node,
    graph_size_bytes: Arc<RwLock<usize>>,
    store: sled::Db,
}

impl SledStorage {
    fn update_stats(&self) {
        let peer_id = self.node.get_peer_id();
        let mut stats = self.node.clone().get("node_stats").get(&peer_id);
        let store = self.store.clone();
        let graph_size_bytes = self.graph_size_bytes.clone();
        /* TODO more performant db size counter then len()
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
        */
    }

    fn handle_get(msg: &Get, my_id: &String, store: &sled::Db, node: &Node) {
        if &msg.from == my_id {
            return;
        }

        let res = unwrap_or_return!(store.get(&msg.node_id.clone()));
        if res.is_none() {
            debug!("have not {}", msg.node_id); return;
        }
        let children = res.unwrap();
        debug!("have {}: {:?}", msg.node_id, children);
        let children = unwrap_or_return!(store.open_tree(&msg.node_id));
        let reply_with_children = match &msg.child_key {
            Some(child_key) => { // reply with specific child if it's found
                let child_val = unwrap_or_return!(children.get(child_key));
                if child_val.is_none() { debug!("requested child {} not found", child_key); return; }
                debug!("requested child {} was found", child_key);
                let child_val = unwrap_or_return!(bincode::deserialize::<NodeData>(&child_val.unwrap()));
                let mut r = BTreeMap::new();
                r.insert(child_key.clone(), child_val);
                r
            },
            None => { // reply with all children of this node
                let mut r = BTreeMap::new();
                for child in children.iter() {
                    let (k,v) = unwrap_or_return!(child);
                    let k = unwrap_or_return!(std::str::from_utf8(&k)).to_string();
                    let v: NodeData = unwrap_or_return!(bincode::deserialize::<NodeData>(&v));
                    r.insert(k, v);
                }
                r
            }
        };
        let mut reply_with_nodes = BTreeMap::new();
        reply_with_nodes.insert(msg.node_id.clone(), reply_with_children);
        let mut recipients = HashSet::new();
        recipients.insert(msg.from.clone());
        let put = Put::new(reply_with_nodes, Some(msg.id.clone()));

        if let Some(addr) = &msg.from_addr {
            addr.try_send(OutgoingMessage { str: put.to_string() });
        } else {
            if let Err(e) = node.get_incoming_msg_sender().try_send(Message::Put(put)) {
                error!("failed to send incoming message to node: {}", e);
            }
        }
    }

    fn handle_put(msg: &Put, my_id: &String, store: &sled::Db) {
        if msg.from == *my_id {
            return;
        }

        for (node_id, update_data) in msg.updated_nodes.iter() {
            debug!("saving k-v {}: {:?}", node_id, update_data);
            // TODO use sled::Tree instead of Children

            let res = unwrap_or_return!(store.get(node_id));
            if let Some(children) = res {
                let children = unwrap_or_return!(store.open_tree(node_id));
                for (child_id, child_data) in update_data {
                    if let Some(existing) = unwrap_or_return!(children.get(child_id)) {
                        let existing = unwrap_or_return!(bincode::deserialize::<NodeData>(&existing));
                        if child_data.updated_at >= existing.updated_at {
                            children.insert(child_id, bincode::serialize(child_data).unwrap());
                        }
                    } else {
                        children.insert(child_id, bincode::serialize(child_data).unwrap());
                    }
                }
            } else {
                store.insert(node_id, vec![1]);
                let children = unwrap_or_return!(store.open_tree(node_id));
                for (child_id, child_data) in update_data {
                    children.insert(child_id, bincode::serialize(child_data).unwrap());
                }
            }
        }
    }
}

#[async_trait]
impl NetworkAdapter for SledStorage {
    fn new(node: Node) -> Self {
        let store = node.config.read().unwrap().sled_config.open().unwrap();
        SledStorage {
            id: "memory_storage".to_string(),
            node,
            graph_size_bytes: Arc::new(RwLock::new(0)),
            store, // If we don't want to store everything in memory, this needs to use something like Redis or LevelDB. Or have a FileSystem adapter for persistence and evict the least important stuff from memory when it's full.
        }
    }

    async fn start(&self) {
        let store = self.store.clone();
        let my_id = self.id.clone();
        let node = self.node.clone();

        /*
        if node.config.read().unwrap().stats {
            self.update_stats();
        }
         */

        let mut rx = node.get_outgoing_msg_receiver();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await {
                    match message {
                        Message::Get(get) => Self::handle_get(&get, &my_id, &store, &node),
                        Message::Put(put) => Self::handle_put(&put, &my_id, &store),
                        _ => {}
                    }
                }
            }
        });
    }
}


