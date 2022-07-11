use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::actor::{Actor, ActorContext};
use crate::message::{Get, Message, Put};
use crate::types::*;
use crate::Config;

use async_trait::async_trait;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sled;
use tokio::time::{sleep, Duration};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

macro_rules! unwrap_or_return {
    ( $e:expr ) => {
        match $e {
            Ok(x) => x,
            Err(e) => {
                error!("{:?}", e);
                return;
            }
        }
    };
}

#[derive(Serialize, Deserialize, Debug)]
struct NodesByPriorityEntry {
    pub node_id: String,
    pub child_key: String,
    pub size: u64,
}

// can be extended later with defaults https://serde.rs/attr-default.html
#[derive(Serialize, Deserialize, Debug)]
struct SledNodeData {
    pub id: u64,
    pub node_id: String,
    pub child_key: String,
    pub data: NodeData,
    pub size: u64,
    pub priority: i32,
    pub last_opened: u64,
    pub times_opened: u32,
}

impl SledNodeData {
    pub fn new(node_id: String, child_key: String, data: NodeData, store: &SledStorage) -> Self {
        let mut s = SledNodeData {
            id: COUNTER.fetch_add(1, Ordering::Relaxed) as u64,
            node_id,
            child_key,
            data,
            size: 0,
            priority: 0,
            last_opened: 0,
            times_opened: 0,
        };
        s.update_size();
        s.update_priority(store, true);
        s
    }

    fn update_size(&mut self) {
        let size = (self.node_id.len() + self.child_key.len() + self.data.value.size()) as u64;
        self.size = size;
    }

    pub fn update_priority(&mut self, storage: &SledStorage, force_index: bool) {
        // TODO update prio only once per sec? or once per 10 times_opened?
        let old_prio = self.priority;
        let mut changed = false;
        if let Some(first_letter) = self.node_id.chars().next() {
            if (first_letter == '~') || (self.node_id == "#") {
                // signed or content addressed data
                let base_prio = match storage.config.my_pub {
                    Some(ref my_pub) => {
                        match self.node_id.find(my_pub) {
                            Some(1) => 110, // prioritize our own stuff. TODO: prioritize our follows also
                            _ => 100,
                        }
                    }
                    _ => 100,
                };
                self.priority = base_prio
                    + ((1000000.0 / (self.size as f64)).log10().max(0.1)
                        * ((self.times_opened + 10) as f64).log10()) as i32;
                debug!(
                    "{}.{} new priority {}",
                    self.node_id, self.child_key, self.priority
                );
                changed = true;
            }
        }
        if !changed {
            self.priority = ((1000000.0 / (self.size as f64)).log10().max(0.1)
                * ((self.times_opened + 10) as f64).log10())
            .min(99.0) as i32;
            debug!(
                "{}.{} new priority {}",
                self.node_id, self.child_key, self.priority
            );
        }
        if force_index || (self.priority != old_prio) {
            // update index
            let index = storage.get_index();
            let old_index_key = format!("{:0>9}{}", old_prio, self.id);
            index
                .remove(&old_index_key)
                .expect("failed to remove old index key");
            let new_index_key = format!("{:0>9}{}", self.priority, self.id);
            debug!("old {} new {}", old_index_key, new_index_key);
            let entry = NodesByPriorityEntry {
                node_id: self.node_id.clone(),
                child_key: self.child_key.clone(),
                size: self.size,
            };
            index
                .insert(new_index_key, bincode::serialize(&entry).unwrap())
                .expect("failed to write into node priority index");
        }
    }
}

#[derive(Clone)]
pub struct SledStorage {
    store: sled::Db,
    meta: sled::Tree,
    max_size: Option<u64>,
    pub config: Config,
}

impl SledStorage {
    pub fn new() -> Self {
        Self::new_with_config(
            Config::default(),
            sled::Config::default().path("sled_db"),
            None,
        )
    }

    pub fn new_with_config(
        config: Config,
        sled_config: sled::Config,
        max_size: Option<u64>,
    ) -> Self {
        let store = sled_config.open().unwrap();
        let meta = store.open_tree("_meta").unwrap();
        meta.set_merge_operator(Self::merge_meta);
        let s = SledStorage {
            config,
            store,
            meta,
            max_size,
        };
        s.change_size(0);
        s
    }

    fn merge_meta(key: &[u8], old_value: Option<&[u8]>, new_value: &[u8]) -> Option<Vec<u8>> {
        let key = std::str::from_utf8(key).unwrap();
        if key == "size" {
            let new_size: u64;
            let new_value = bincode::deserialize::<i64>(new_value).unwrap();
            if let Some(old_value) = old_value {
                let old_value = bincode::deserialize::<u64>(old_value).unwrap();
                new_size = ((old_value as i64) + new_value).max(0) as u64;
            } else {
                new_size = new_value.max(0) as u64;
            }
            return Some(bincode::serialize(&new_size).unwrap());
        }
        Some(new_value.to_vec())
    }

    pub fn get_size(&self) -> Result<u64, &'static str> {
        match self.meta.get(b"size") {
            Ok(size) => {
                let size = size.ok_or("no size found")?;
                bincode::deserialize::<u64>(&size).or(Err("could not deserialize db size"))
            }
            _ => Err("sled read failed"),
        }
    }

    fn change_size(&self, change: i64) {
        self.meta
            .merge(b"size", bincode::serialize(&change).unwrap())
            .expect("sled db size update failed");
    }

    fn open_child(
        &self,
        node_id: &String,
        child_key: &String,
        bytes: &[u8],
        update_times_opened: bool,
    ) -> Result<SledNodeData, ()> {
        let mut res = match bincode::deserialize::<SledNodeData>(bytes) {
            Ok(v) => Ok(v),
            _ => {
                // I can see future migrations becoming a problem
                match bincode::deserialize::<NodeData>(bytes) {
                    Ok(v) => Ok(SledNodeData::new(
                        node_id.clone(),
                        child_key.clone(),
                        v,
                        &self,
                    )), // migrate,
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
                }
                _ => {}
            }
        }
        res
    }

    fn get_index(&self) -> sled::Tree {
        // sqlite could do the indexing automatically
        self.store.open_tree("_nodes_by_priority").unwrap()
    }

    fn handle_get(&self, get: Get, ctx: &ActorContext) {
        let res = unwrap_or_return!(self.store.get(&get.node_id.clone()));
        if res.is_none() {
            debug!("have not {}", get.node_id);
            return;
        }
        let children = res.unwrap();
        debug!("have {}: {:?}", get.node_id, children);
        let children = unwrap_or_return!(self.store.open_tree(&get.node_id));
        let reply_with_children = match &get.child_key {
            Some(child_key) => {
                // reply with specific child if it's found
                let child_val = unwrap_or_return!(children.get(child_key));
                if child_val.is_none() {
                    debug!("requested child {} not found", child_key);
                    return;
                }
                debug!("requested child {} was found", child_key);
                let child_val = unwrap_or_return!(self.open_child(
                    &get.node_id,
                    child_key,
                    &child_val.unwrap(),
                    true
                ));
                let _ =
                    children.insert(get.node_id.clone(), bincode::serialize(&child_val).unwrap());
                let mut r = BTreeMap::new();
                r.insert(child_key.clone(), child_val.data);
                r
            }
            None => {
                // reply with all children of this node
                let mut r = BTreeMap::new();
                for child in children.iter() {
                    let (k, v) = unwrap_or_return!(child);
                    let k = unwrap_or_return!(std::str::from_utf8(&k)).to_string();
                    let v: SledNodeData =
                        unwrap_or_return!(self.open_child(&get.node_id, &k, &v, true));
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
        let mut put = Put::new(reply_with_nodes, Some(get.id.clone()), my_addr);
        put.to_string();
        if put.checksum != get.checksum {
            let _ = get.from.send(Message::Put(put));
        } else {
            debug!("put.checksum == get.checksum, not replying with the same data");
        }
    }

    fn handle_put(&self, put: Put) {
        for (node_id, update_data) in put.updated_nodes.into_iter().rev() {
            if (node_id.len() > 0) && node_id.chars().next().unwrap() == '_' {
                continue; // _ paths reserved for our metadata. we can add escaping if needed.
            }
            debug!("saving k-v {}: {:?}", node_id, update_data);
            let res = unwrap_or_return!(self.store.get(&node_id));
            if let Some(_children) = res {
                let children = unwrap_or_return!(self.store.open_tree(&node_id));
                for (child_id, child_data) in update_data {
                    if let Some(existing) = unwrap_or_return!(children.get(&child_id)) {
                        let mut existing: SledNodeData = unwrap_or_return!(
                            self.open_child(&node_id, &child_id, &existing, false)
                        );
                        if child_data.updated_at >= existing.data.updated_at {
                            existing.data = child_data;
                            let old_size = existing.size as i64;
                            existing.update_size();
                            existing.update_priority(&self, false);
                            let _ = children
                                .insert(child_id.clone(), bincode::serialize(&existing).unwrap());
                            self.change_size((existing.size as i64) - old_size);
                        }
                    } else {
                        let child =
                            SledNodeData::new(node_id.clone(), child_id.clone(), child_data, &self);
                        let _ = children.insert(child_id, bincode::serialize(&child).unwrap());
                        self.change_size(child.size as i64);
                    }
                }
            } else {
                let _ = self.store.insert(node_id.clone(), vec![1]);
                let children = unwrap_or_return!(self.store.open_tree(&node_id));
                for (child_id, child_data) in update_data {
                    let child =
                        SledNodeData::new(node_id.clone(), child_id.clone(), child_data, &self);
                    let _ = children.insert(child_id, bincode::serialize(&child).unwrap());
                    self.change_size(child.size as i64);
                }
            }
        }
    }

    fn evict(&self, amount: u64) {
        let mut evicted = 0;
        let index = self.store.open_tree("_nodes_by_priority").unwrap();
        while evicted < amount {
            match index.pop_min() {
                // TODO transaction? don't pop unless the actual data was removed
                Ok(opt) => {
                    if let Some((_key, entry)) = opt {
                        let entry = bincode::deserialize::<NodesByPriorityEntry>(&entry).unwrap();
                        let node = unwrap_or_return!(self.store.open_tree(&entry.node_id));
                        node.remove(&entry.child_key).expect("evict failed");
                        self.change_size(0 - (entry.size as i64));
                        evicted += entry.size;
                    }
                }
                _ => break,
            }
        }
        debug!("evict done");
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
        if let Some(limit) = self.max_size {
            let storage = self.clone();
            ctx.child_task(async move {
                loop {
                    sleep(Duration::from_millis(2000)).await;
                    match storage.get_size() {
                        Ok(size) => {
                            if size >= limit {
                                let excess = size - limit;
                                info!(
                                    "sled db too big ({} bytes), evicting lowest priority data",
                                    size
                                );
                                storage.evict(excess); // TODO: size_on_disk() not shrinking (enough)
                                sleep(Duration::from_millis(1000)).await;
                                let new_size = storage.get_size().unwrap();
                                info!("size after eviction {} (was {} bytes)", new_size, size);
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    }

    async fn stopping(&mut self, _context: &ActorContext) {
        info!("SledStorage adapter stopping");
    }
}

#[cfg(test)]
mod tests {
    use crate::actor::{ActorContext, Addr};
    use crate::adapters::sled_storage::SledStorage;
    use crate::message::{Get, Message, Put};
    use crate::types::{Children, NodeData, Value};
    use crate::Config;
    use tokio::sync::mpsc::unbounded_channel;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn should_not_reply_with_put_when_checksum_same_as_in_get() {
        let path = std::path::Path::new("./sled_checksum_test_db");
        let (sender, mut receiver) = unbounded_channel::<Message>();
        let return_addr = Addr::new(sender);
        let sled_storage = SledStorage::new_with_config(
            Config::default(),
            sled::Config::default().path(path),
            None,
        );
        let mut ctx = ActorContext::new("peer_id".to_string());
        let sled_addr = ctx.start_actor(Box::new(sled_storage));

        let mut children = Children::default();
        children.insert(
            "about".to_string(),
            NodeData {
                value: Value::Text("Jesus Built My Hot Rod".to_string()),
                updated_at: 0 as f64,
            },
        );
        let mut put = Put::new_from_kv("profile".to_string(), children, return_addr.clone());
        put.to_string();
        let checksum = put.checksum.clone().unwrap();
        sled_addr.send(Message::Put(put)).ok();
        sleep(Duration::from_millis(10)).await;
        let mut get = Get::new(
            "profile".to_string(),
            Some("about".to_string()),
            return_addr.clone(),
        );

        get.checksum = Some(checksum);
        sled_addr.send(Message::Get(get.clone())).ok();
        sleep(Duration::from_millis(10)).await;
        assert!(receiver.try_recv().is_err());

        get.checksum = None;
        sled_addr.send(Message::Get(get)).ok();
        sleep(Duration::from_millis(10)).await;
        assert!(receiver.try_recv().is_ok());

        std::fs::remove_dir_all(path).ok();

        ctx.stop();
    }
}
