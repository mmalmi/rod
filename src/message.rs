use serde_json::{json, Value as SerdeJsonValue};
use crate::utils::random_string;
use std::collections::HashSet;
use crate::types::*;

#[derive(Clone, Debug)]
enum NetworkMessage {
    Get { id: String, node_id: String, child_name: Option<String> },
    Put { id: String, in_response_to: String, updated_fields: HashMap<String, NodeData>, checksum: String },
    Hi { from: String },
    Other { id: String, string: String }
}

impl NetworkMessage {
    fn get_into_string(msg: Self::Get) -> String {

    }

    fn put_into_string(msg: Self::Put) -> String {

    }

    fn hi_into_string(msg: Self::Hi) -> String {

    }
}

impl TryFrom<&str> for NetworkMessage {
    type Error = &'static str;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let json: SerdeJsonValue = match serde_json::from_str(s) {
            Ok(json) => json,
            Err(_) => { return Err("Failed to parse message as JSON"); }
        };

        Self::Hi { from: "".to_string() }
    }
}

impl Into<String> for NetworkMessage {
    fn into(&self) -> String {
        match self {
            Self::Get => Self::get_into_string(self),
            Self::Put => Self::put_into_string(self),
            Self::Hi => Self::hi_into_string(self),
            Self::Other => self.string
        }
    }
}