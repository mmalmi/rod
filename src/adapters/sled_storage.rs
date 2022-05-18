use std::collections::{HashSet, BTreeMap};

use crate::message::{Message, Put, Get};
use crate::actor::{Actor, ActorContext};
use crate::Config;
use crate::types::*;

use tokio::time::{Duration, sleep};
use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use log::{debug, error, info};
use sled;

macro_rules! unwrap_or_return {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                error!("{:?}", e);
                return;
            },
        }
    }
}

// can be extended later with defaults https://serde.rs/attr-default.html
#[derive(Serialize, Deserialize, Debug)]
struct SledNodeData {
    pub node_id: String,
    pub child_key: String,
    pub data: NodeData,
    pub size: u64,
    pub priority: i32,
    pub last_opened: u64,
    pub times_opened: u32
}

impl SledNodeData {
    pub fn new(node_id: String, child_key: String, data: NodeData, store: &SledStorage) -> Self {
        let size = (child_key.len() + data.value.size()) as u64;
        let mut s = SledNodeData {
            node_id,
            child_key,
            data,
            size,
            priority: 0,
            last_opened: 0,
            times_opened: 0
        };
        s.update_priority(store, true);
        s
    }

    pub fn update_priority(&mut self, store: &SledStorage, force_index: bool) { // TODO update prio only once per sec? or once per 10 times_opened?
        let old_prio = self.priority; // handle case where
        let mut changed = false;
        if let Some(first_letter) = self.node_id.chars().next() {
            if (first_letter == '~') || (self.node_id == "#") { // signed or content addressed data
                let base_prio = match store.config.my_pub {
                    Some(ref my_pub) => {
                        match self.node_id.find(my_pub) {
                            Some(1) => 110, // prioritize our own stuff. TODO: prioritize our follows also
                            _ => 100
                        }
                    },
                    _ => 100
                };
                self.priority = base_prio + ((1000000.0 / (self.size as f64)).log10().max(0.1) * ((self.times_opened + 10) as f64).log10()) as i32;
                debug!("{}.{} new priority {}", self.node_id, self.child_key, self.priority);
                changed = true;
            }
        }
        if !changed {
            self.priority = ((1000000.0 / (self.size as f64)).log10().max(0.1) * ((self.times_opened + 10) as f64).log10()).min(99.0) as i32;
            debug!("{}.{} new priority {}", self.node_id, self.child_key, self.priority);
        }
        if force_index || (self.priority != old_prio) {
            // update index
            let index = store.get_index();
            let node_id = str::replace(&self.node_id, ":", "::");
            let child_key = str::replace(&self.child_key, ":", "::");
            let old_index_key = format!("{:0>9}{}:{}", old_prio, node_id, child_key);
            index.remove(&old_index_key);
            let new_index_key = format!("{:0>9}{}:{}", self.priority, node_id, child_key);
            debug!("old {} new {}", old_index_key, new_index_key);
            index.insert(new_index_key, vec![1]);
        }
    }
}

pub struct SledStorage {
    pub config: Config,
    store: sled::Db,
}

impl SledStorage {
    pub fn new(config: Config) -> Self {
        let store = config.sled_config.open().unwrap();
        SledStorage {
            config,
            store
        }
    }

    fn open_child(&self, node_id: &String, child_key: &String, bytes: &[u8], update_times_opened: bool) -> Result<SledNodeData, ()> {
        let mut res = match bincode::deserialize::<SledNodeData>(bytes) {
            Ok(v) => Ok(v),
            _ => {
                // I can see future migrations becoming a problem
                match bincode::deserialize::<NodeData>(bytes) {
                    Ok(v) => Ok(SledNodeData::new(node_id.clone(), child_key.clone(), v, &self)), // migrate,
                    _ => {
                        error!("could not deserialize data from sled");
                        Err(())
                    }
                }
            }
        };
        if update_times_opened {
            match res {
                Ok(ref mut node) => {
                    node.times_opened += 1;
                    node.update_priority(&self, false);
                },
                _ => {}
            }
        }
        res
    }

    fn get_index(&self) -> sled::Tree {
        self.store.open_tree("_nodes_by_priority").unwrap()
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
                let child_val = unwrap_or_return!(self.open_child(&get.node_id, child_key, &child_val.unwrap(), true));
                let _ = children.insert(get.node_id.clone(), bincode::serialize(&child_val).unwrap());
                let mut r = BTreeMap::new();
                r.insert(child_key.clone(), child_val.data);
                r
            },
            None => { // reply with all children of this node
                let mut r = BTreeMap::new();
                for child in children.iter() {
                    let (k,v) = unwrap_or_return!(child);
                    let k = unwrap_or_return!(std::str::from_utf8(&k)).to_string();
                    let v: SledNodeData = unwrap_or_return!(self.open_child(&get.node_id, &k, &v, true));
                    let _ = children.insert(get.node_id.clone(), bincode::serialize(&v).unwrap());
                    r.insert(k, v.data);
                }
                r
            }
        };
        let mut reply_with_nodes = BTreeMap::new();
        reply_with_nodes.insert(get.node_id.clone(), reply_with_children);
        let mut recipients = HashSet::new();
        recipients.insert(get.from.clone());
        debug!("direct replying to {}", get.from);
        let my_addr = ctx.addr.clone();
        let put = Put::new(reply_with_nodes, Some(get.id.clone()), my_addr);
        let _ = get.from.sender.send(Message::Put(put));
    }

    fn handle_put(&self, put: Put) {
        for (node_id, update_data) in put.updated_nodes.into_iter().rev() {
            debug!("saving k-v {}: {:?}", node_id, update_data);
            let res = unwrap_or_return!(self.store.get(&node_id));
            if let Some(_children) = res {
                let children = unwrap_or_return!(self.store.open_tree(&node_id));
                for (child_id, child_data) in update_data {
                    if let Some(existing) = unwrap_or_return!(children.get(&child_id)) {
                        let mut existing: SledNodeData = unwrap_or_return!(self.open_child(&node_id, &child_id, &existing, false));
                        if child_data.updated_at >= existing.data.updated_at {
                            existing.data = child_data;
                            existing.update_priority(&self, false);
                            let _ = children.insert(child_id.clone(), bincode::serialize(&existing).unwrap());
                        }
                    } else {
                        let child = SledNodeData::new(node_id.clone(), child_id.clone(), child_data, &self);
                        let _ = children.insert(child_id, bincode::serialize(&child).unwrap());
                    }
                }
            } else {
                let _ = self.store.insert(node_id.clone(), vec![1]);
                let children = unwrap_or_return!(self.store.open_tree(&node_id));
                for (child_id, child_data) in update_data {
                    let child = SledNodeData::new(node_id.clone(), child_id.clone(), child_data, &self);
                    let _ = children.insert(child_id, bincode::serialize(&child).unwrap());
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
            Message::Put(put) => self.handle_put(put),
            _ => {}
        }
    }

    async fn pre_start(&mut self, ctx: &ActorContext) {
        info!("SledStorage adapter starting");
        if let Some(limit) = self.config.sled_storage_limit.clone() {
            let store = self.store.clone();
            ctx.abort_on_stop(tokio::spawn(async move {
                loop {
                    sleep(Duration::from_millis(500)).await;
                    match store.size_on_disk() {
                        Ok(size) => {
                            if size >= limit {
                                debug!("sled db too big, evicting lowest priority data");
                                // TODO evict
                            }
                        },
                        _ => {}
                    }
                }
            }));
        }
    }

    async fn stopping(&mut self, _context: &ActorContext) {
        info!("SledStorage adapter stopping");
    }
}


