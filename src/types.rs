use serde::{Serialize, Deserialize};
use std::collections::BTreeMap;
use serde_json::{json, Value as SerdeJsonValue};
use std::convert::TryFrom;

/// Branch node
pub type Children = BTreeMap<String, NodeData>;

/// Data in a leaf node
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeData {
    pub value: GunValue,
    pub updated_at: f64
}

impl NodeData {
    pub fn default() -> Self {
        Self {
            value: GunValue::Null,
            updated_at: 0.0
        }
    }
}

/// Value types supported by gun.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum GunValue {
    Null,
    Bit(bool),
    Number(f64),
    Text(String),
    Link(String),
}

impl GunValue {
    pub fn size(&self) -> usize {
        match self {
            GunValue::Text(s) => s.len(),
            _ => std::mem::size_of_val(self)
        }
    }
}

impl TryFrom<SerdeJsonValue> for GunValue {
    type Error = &'static str;

    fn try_from(v: SerdeJsonValue) -> Result<GunValue, Self::Error> {
        match v {
            SerdeJsonValue::Null => Ok(GunValue::Null),
            SerdeJsonValue::Bool(b) => Ok(GunValue::Bit(b)),
            SerdeJsonValue::String(s) => Ok(GunValue::Text(s)),
            SerdeJsonValue::Number(n) => {
                match n.as_f64() {
                    Some(n) => Ok(GunValue::Number(n)),
                    _ => Err("not convertible to f64")
                }
            },
            SerdeJsonValue::Object(_) => Err("cannot convert json object into GunValue"),
            SerdeJsonValue::Array(_) => Err("cannot convert array into GunValue")
        }
    }
}

impl From<GunValue> for SerdeJsonValue {
    fn from (v: GunValue) -> SerdeJsonValue {
        match v {
            GunValue::Null => SerdeJsonValue::Null,
            GunValue::Text(t) => SerdeJsonValue::String(t),
            GunValue::Bit(b) => SerdeJsonValue::Bool(b),
            GunValue::Number(n) => json!(n),
            GunValue::Link(l) => SerdeJsonValue::String(l) // TODO fix. Object?
        }
    }
}

impl From<usize> for GunValue {
    fn from(n: usize) -> GunValue {
        GunValue::Number(n as f64)
    }
}

impl From<f32> for GunValue {
    fn from(n: f32) -> GunValue {
        GunValue::Number(n as f64)
    }
}

impl From<u64> for GunValue {
    fn from(n: u64) -> GunValue {
        GunValue::Number(n as f64)
    }
}

impl From<&str> for GunValue {
    fn from(s: &str) -> GunValue {
        GunValue::Text(s.to_string())
    }
}

impl From<String> for GunValue {
    fn from(s: String) -> GunValue {
        GunValue::Text(s)
    }
}
