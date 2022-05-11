use std::collections::{HashSet, BTreeMap};

use crate::message::{Message, Put, Get};
use crate::actor::{Actor, ActorContext};
use crate::Config;
use crate::types::*;

use async_trait::async_trait;
use log::{debug, error, info};
use sled;

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
    config: Config,
    store: sled::Db,
}

impl SledStorage {
    pub fn new(config: Config) -> Self {
        let store = config.sled_config.open().unwrap();
        SledStorage {
            config,
            store, // If we don't want to store everything in memory, this needs to use something like Redis or LevelDB. Or have a FileSystem adapter for persistence and evict the least important stuff from memory when it's full.
        }
    }

    fn handle_get(&self, get: Get, ctx: &ActorContext) {
        let res = unwrap_or_return!(self.store.get(&get.node_id.clone()));
        if res.is_none() {
            debug!("have not {}", get.node_id); return;
        }
        let children = res.unwrap();
        debug!("have {}: {:?}", get.node_id, children);
        let children = unwrap_or_return!(self.store.open_tree(&get.node_id));
        let reply_with_children = match &get.child_key {
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
        reply_with_nodes.insert(get.node_id.clone(), reply_with_children);
        let mut recipients = HashSet::new();
        recipients.insert(get.from.clone());
        debug!("direct replying to {}", get.from);
        let my_addr = (*ctx.addr.upgrade().unwrap()).clone();
        let put = Put::new(reply_with_nodes, Some(get.id.clone()), my_addr);
        let _ = get.from.sender.send(Message::Put(put));
    }

    fn handle_put(&self, put: Put, ctx: &ActorContext) {
        for (node_id, update_data) in put.updated_nodes.iter().rev() {
            debug!("saving k-v {}: {:?}", node_id, update_data);
            // TODO use sled::Tree instead of Children

            let res = unwrap_or_return!(self.store.get(node_id));
            if let Some(children) = res {
                let children = unwrap_or_return!(self.store.open_tree(node_id));
                for (child_id, child_data) in update_data {
                    if let Some(existing) = unwrap_or_return!(children.get(child_id)) {
                        let existing = unwrap_or_return!(bincode::deserialize::<NodeData>(&existing));
                        if child_data.updated_at >= existing.updated_at {
                            let _ = children.insert(child_id, bincode::serialize(child_data).unwrap());
                        }
                    } else {
                        let _ = children.insert(child_id, bincode::serialize(child_data).unwrap());
                    }
                }
            } else {
                let _ = self.store.insert(node_id, vec![1]);
                let children = unwrap_or_return!(self.store.open_tree(node_id));
                for (child_id, child_data) in update_data {
                    let _ = children.insert(child_id, bincode::serialize(child_data).unwrap());
                }
            }
        }
    }
}

#[async_trait]
impl Actor for SledStorage {
    async fn handle(&mut self, message: Message, ctx: &ActorContext) {
        debug!("SledStorage incoming message {:?}", message);
        match message {
            Message::Get(get) => self.handle_get(get, ctx),
            Message::Put(put) => self.handle_put(put, ctx),
            _ => {}
        }
    }

    async fn pre_start(&mut self, _context: &ActorContext) {
        info!("SledStorage adapter starting");
    }
}


