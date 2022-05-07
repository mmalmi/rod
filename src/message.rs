use serde_json::{json, Value as SerdeJsonValue};
use crate::utils::random_string;
use std::collections::{HashSet, BTreeMap};
use crate::types::*;
use log::{debug, error};
use std::convert::TryFrom;
use actix::Addr;
use crate::adapters::websocket_server::MyWs;

#[derive(Clone, Debug)]
pub struct Get {
    pub id: String,
    pub from: String,
    pub from_addr: Option<Addr<MyWs>>,
    pub recipients: Option<HashSet<String>>,
    pub node_id: String,
    pub child_key: Option<String>,
    pub json_str: Option<String>
}
impl Get {
    pub fn new(node_id: String, child_key: Option<String>, from: String) -> Self {
        Self {
            id: random_string(8),
            from,
            from_addr: None,
            recipients: None,
            node_id,
            child_key,
            json_str: None
        }
    }

    pub fn to_string(&self) -> String {
        if let Some(json_str) = self.json_str.clone() {
            return json_str;
        }

        let mut json = json!({
            "get": {
                "#": &self.node_id
            },
            "#": &self.id
        });
        if let Some(child_key) = self.child_key.clone() {
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
    pub updated_nodes: BTreeMap<String, Children>,
    pub checksum: Option<String>,
    pub json_str: Option<String>
}
impl Put {
    pub fn new(updated_nodes: BTreeMap<String, Children>, in_response_to: Option<String>) -> Self {
        Self {
            id: random_string(8),
            from: "".to_string(),
            recipients: None,
            in_response_to,
            updated_nodes,
            checksum: None,
            json_str: None
        }
    }

    pub fn new_from_kv(key: String, children: Children) -> Self {
        let mut updated_nodes = BTreeMap::new();
        updated_nodes.insert(key, children);
        Put::new(updated_nodes, None)
    }

    pub fn to_string(&self) -> String {
        if let Some(json_str) = self.json_str.clone() {
            return json_str;
        }

        let mut json = json!({
            "put": {},
            "#": self.id.to_string(),
        });

        if let Some(in_response_to) = &self.in_response_to {
            json["@"] = json!(in_response_to);
        }

        if let Some(checksum) = &self.checksum {
            json["##"] = json!(checksum);
        }

        for (node_id, children) in self.updated_nodes.iter() {
            let node = &mut json["put"][node_id];
            node["_"] = json!({
                "#": node_id,
                ">": {}
            });
            for (k, v) in children.iter() {
                node["_"][">"][k] = json!(v.updated_at);
                node[k] = v.value.clone().into();
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

    fn from_put_obj(json: &SerdeJsonValue, json_str: String, msg_id: String, from: String) -> Result<Self, &'static str> {
        let obj = match json.get("put").unwrap().as_object() {
            Some(obj) => obj,
            _ => { return Err("invalid message: msg.put was not an object"); }
        };
        let in_response_to = match json.get("@") {
            Some(in_response_to) => match in_response_to.as_str() {
                Some(in_response_to) => Some(in_response_to.to_string()),
                _ => { return Err("message @ field was not a string"); }
            },
            _ => None
        };
        let checksum = match json.get("##") {
            Some(checksum) => Some(checksum.to_string()),
            _ => None
        };
        let mut updated_nodes = BTreeMap::<String, Children>::new();
        for (node_id, node_data) in obj.iter() {
            let node_data = match node_data.as_object() {
                Some(obj) => obj,
                _ => { return Err("put node data was not an object"); }
            };
            let updated_at_times = match node_data["_"][">"].as_object() {
                Some(obj) => obj,
                _ => { return Err("no metadata _ in Put node object"); }
            };
            let mut children = Children::default();
            for (child_key, child_val) in node_data.iter() {
                if child_key == "_" { continue; }
                let updated_at = match updated_at_times.get(child_key) {
                    Some(updated_at) => updated_at,
                    _ => { return Err("no updated_at found for Put key"); }
                };
                let updated_at = match updated_at.as_f64() {
                    Some(val) => val,
                    None => { return Err("updated_at was not a number"); }
                };
                let value = match GunValue::try_from(child_val.clone()) {
                    Ok(v) => v,
                    Err(e) => { return Err(e) }
                };
                children.insert(child_key.to_string(), NodeData { updated_at, value });
            }
            updated_nodes.insert(node_id.to_string(), children);
        }
        let put = Put {
            id: msg_id.to_string(),
            from,
            recipients: None,
            in_response_to,
            updated_nodes,
            checksum,
            json_str: Some(json_str)
        };
        Ok(Message::Put(put))
    }

    fn from_get_obj(json: &SerdeJsonValue, json_str: String, msg_id: String, from: String) -> Result<Self, &'static str> {
        /* TODO: other types of child_key selectors than equality.

        node.get({'.': {'<': cursor, '-': true}, '%': 20 * 1000}).once().map().on((value, key) => { ...

        '*' wildcard selector

         */

        let get = json.get("get").unwrap();
        let node_id = match get["#"].as_str() {
            Some(str) => str,
            _ => { return Err("no node id (#) found in get message"); }
        };
        let child_key = match get.get(".") {
            Some(child_key) => match child_key.as_str() {
                Some(child_key) => Some(child_key.to_string()),
                _ => { return Err("get child_key . was not a string") }
            },
            _ => None
        };
        debug!("get node_id {}", node_id);
        let msg_id = msg_id.replace("\"", "");
        let get = Get {
            id: msg_id,
            from,
            from_addr: None,
            recipients: None,
            node_id: node_id.to_string(),
            child_key,
            json_str: Some(json_str)
        };
        Ok(Message::Get(get))
    }
    
    pub fn from_json_obj(json: &SerdeJsonValue, json_str: String, from: String) -> Result<Self, &'static str> {
        let obj = match json.as_object() {
            Some(obj) => obj,
            _ => { return Err("not a json object"); }
        };
        let msg_id = match obj["#"].as_str() {
            Some(str) => str.to_string(),
            _ => { return Err("msg id not a string"); }
        };
        if msg_id.len() > 32 {
            return Err("msg id too long (> 32)");
        }
        if !msg_id.chars().all(char::is_alphanumeric) {
            return Err("msg_id must be alphanumeric");
        }
        if obj.contains_key("put") {
            Self::from_put_obj(json, json_str, msg_id, from)
        } else if obj.contains_key("get") {
            Self::from_get_obj(json, json_str, msg_id, from)
        } else if let Some(_dam) = obj.get("dam") {
            Ok(Message::Hi { from: msg_id })
        } else {
            Err("Unrecognized message")
        }
    }

    pub fn try_from(s: &str, from: String) -> Result<Vec<Self>, &str> {
        let json: SerdeJsonValue = match serde_json::from_str(s) {
            Ok(json) => json,
            Err(_) => { return Err("Failed to parse message as JSON"); }
        };
        
        if let Some(arr) = json.as_array() {
            let mut vec = Vec::<Self>::new();
            for msg in arr {
                vec.push(Self::from_json_obj(msg, msg.to_string(), from.clone())?);
            }
            Ok(vec)
        } else {
            match Self::from_json_obj(&json, s.to_string(), from) {
                Ok(msg) => Ok(vec![msg]),
                Err(e) => Err(e)
            }
        }
    }
}