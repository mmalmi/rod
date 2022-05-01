use serde_json::{json, Value as SerdeJsonValue};
use crate::utils::random_string;
use std::collections::{HashSet, BTreeMap};
use crate::types::*;

#[derive(Clone, Debug)]
pub struct Get {
    pub id: String,
    pub from: String,
    pub recipients: Option<HashSet<String>>,
    pub node_id: String,
    pub child_key: Option<String>
}
impl Get {
    pub fn new(node_id: String, child_key: Option<String>) -> Self {
        Self {
            id: random_string(8),
            from: "".to_string(),
            recipients: None,
            node_id,
            child_key
        }
    }

    pub fn to_string(&self) -> String {
        let mut json = json!({
            "get": {
                "#": self.node_id
            },
            "#": self.id
        });
        if let Some(child_key) = self.child_key {
            json["get"]["."] = json!(child_key);
        }
        json.to_string()
    }
}

#[derive(Clone, Debug)]
pub struct Put {
    pub id: String,
    pub from: String,
    pub recipients: Option<HashSet<String>>,
    pub in_response_to: Option<String>,
    pub updated_nodes: BTreeMap<String, NodeData>,
    pub checksum: Option<String>
}
impl Put {
    pub fn new(updated_nodes: BTreeMap<String, NodeData>, in_response_to: Option<String>) -> Self {
        Self {
            id: random_string(8),
            from: "".to_string(),
            recipients: None,
            in_response_to,
            updated_nodes,
            checksum: None
        }
    }

    pub fn new_from_kv(key: String, data: NodeData) -> Self {
        let updated_nodes = BTreeMap::new();
        updated_nodes.insert(key, data);
        Put::new(updated_nodes, None)
    }

    pub fn to_string(&self) -> String {
        let mut json = json!({
            "put": {},
            "#": self.id,
        });

        for (node_id, node_data) in self.updated_nodes.iter() {
            let mut node = json["put"][node_id];
            node["_"] = json!({
                "#": node_id,
                ">": {}
            });
            match node_data.value {
                GunValue::Children(children) => {
                    for (k, v) in children.iter() {
                        node[">"][k] = json!(node_data.updated_at);
                        node[k] = json!(v);
                    }
                },
                _ => {}
            }
        }
        json.to_string()
    }
}

#[derive(Clone, Debug)]
pub enum Message { // could consider structs for each enum variant (Message::Get(Get))
    Get(Get),
    Put(Put),
    Hi { from: String }
}

impl Message {
    pub fn to_string(self) -> String {
        match self {
            Message::Get(get) => get.to_string(),
            Message::Put(put) => put.to_string(),
            Message::Hi { from } => json!({"dam": "hi","#": from}).to_string()
        }
    }

    pub fn try_from(s: &str) -> Result<Self, &str> {
        let json: SerdeJsonValue = match serde_json::from_str(s) {
            Ok(json) => json,
            Err(_) => { return Err("Failed to parse message as JSON"); }
        };

        Ok(Message::Hi { from: "".to_string() })
    }
}