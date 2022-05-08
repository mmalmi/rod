use std::collections::{HashSet, BTreeMap};

use crate::message::{Message, Put, Get};
use crate::actor::Actor;
use crate::Node;
use crate::types::*;

use async_trait::async_trait;
use log::{debug, error};
use tokio::sync::mpsc::Receiver;
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
    receiver: Receiver<Message>,
    store: sled::Db,
}

impl SledStorage {
    fn handle_get(&self, msg: Get) {
        if &msg.from == my_id {
            return;
        }

        let res = unwrap_or_return!(self.store.get(&msg.node_id.clone()));
        if res.is_none() {
            debug!("have not {}", msg.node_id); return;
        }
        let children = res.unwrap();
        debug!("have {}: {:?}", msg.node_id, children);
        let children = unwrap_or_return!(self.store.open_tree(&msg.node_id));
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
            if let Err(e) = self.node.get_router_addr().sender.try_send(Message::Put(put)) {
                error!("failed to send incoming message to node: {}", e);
            }
        }
    }

    fn handle_put(&self, msg: Put) {
        if msg.from == *my_id {
            return;
        }

        for (node_id, update_data) in msg.updated_nodes.iter().rev() {
            debug!("saving k-v {}: {:?}", node_id, update_data);
            // TODO use sled::Tree instead of Children

            let res = unwrap_or_return!(self.store.get(node_id));
            if let Some(children) = res {
                let children = unwrap_or_return!(self.store.open_tree(node_id));
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
                self.store.insert(node_id, vec![1]);
                let children = unwrap_or_return!(self.store.open_tree(node_id));
                for (child_id, child_data) in update_data {
                    children.insert(child_id, bincode::serialize(child_data).unwrap());
                }
            }
        }
    }
}

#[async_trait]
impl Actor for SledStorage {
    fn new(receiver: Receiver<Message>, node: Node) -> Self {
        let store = node.config.read().unwrap().sled_config.open().unwrap();
        SledStorage {
            id: "memory_storage".to_string(),
            receiver,
            node,
            store, // If we don't want to store everything in memory, this needs to use something like Redis or LevelDB. Or have a FileSystem adapter for persistence and evict the least important stuff from memory when it's full.
        }
    }

    async fn start(&self) {
        while let Some(message) = self.receiver.recv().await {
            match message {
                Message::Get(get) => self.handle_get(get),
                Message::Put(put) => self.handle_put(put),
                _ => {}
            }
        }
    }
}


