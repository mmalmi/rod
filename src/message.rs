use serde_json::{json, Value as SerdeJsonValue};
use crate::utils::random_string;
use std::collections::{HashSet, HashMap};
use crate::types::*;
use std::convert::TryFrom;

#[derive(Clone, Debug)]
struct Get { id: String, from: String, recipients: HashSet<String>, node_id: String, child_key: Option<String> }
impl Get {
    fn new(node_id: String, child_key: Option<String>) -> Self {
        Self {
            id: random_string(8),
            from: "".to_string(),
            recipients: HashSet::new(),
            node_id,
            child_key
        }
    }

    fn to_string(&self) -> String {
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
struct Put { id: String, from: String, recipients: HashSet<String>, in_response_to: String, updated_nodes: HashMap<String, NodeData>, checksum: String }
impl Put {
    fn new(updated_nodes: HashMap<String, NodeData>, in_response_to: String) -> Self {
        Self {
            id: random_string(8),
            from: "".to_string(),
            recipients: HashSet::new(),
            in_response_to,
            updated_nodes,
            checksum: "".to_string()
        }
    }

    fn to_string(&self) -> String {
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
                        node[">"][k] = message::SerdeJsonValue::Number(node_data.updated_at);
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

impl TryFrom<&str> for Message {
    type Error = &'static str;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let json: SerdeJsonValue = match serde_json::from_str(s) {
            Ok(json) => json,
            Err(_) => { return Err("Failed to parse message as JSON"); }
        };

        Ok(Message::Hi { from: "".to_string() })
    }
}

impl Into<String> for Message {
    fn into(self) -> String {
        match self {
            Message::Get(get) => get.to_string(),
            Message::Put(put) => put.to_string(),
            Message::Hi { from } => json!({"dam": "hi","#": from}).to_string()
        }
    }
}