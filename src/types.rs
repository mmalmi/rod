use serde::{Deserialize, Serialize};
use serde_json::{json, Value as SerdeJsonValue};
use std::collections::BTreeMap;
use std::convert::TryFrom;

/// Branch node
pub type Children = BTreeMap<String, NodeData>;

/// Data in a leaf node
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NodeData {
    pub value: Value,
    pub updated_at: f64,
}

impl NodeData {
    pub fn default() -> Self {
        Self {
            value: Value::Null,
            updated_at: 0.0,
        }
    }
}

/// Value types supported by rod & gun.js.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Value {
    Null,
    Bit(bool),
    Number(f64),
    Text(String),
    Link(String),
}

impl Value {
    pub fn size(&self) -> usize {
        match self {
            Value::Text(s) => s.len(),
            _ => std::mem::size_of_val(self),
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Value::Null => "null".to_string(),
            Value::Bit(bool) => {
                if *bool {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Value::Number(n) => n.to_string(),
            Value::Text(t) => t.clone(),
            Value::Link(l) => l.clone(),
        }
    }
}

impl TryFrom<SerdeJsonValue> for Value {
    type Error = &'static str;

    fn try_from(v: SerdeJsonValue) -> Result<Value, Self::Error> {
        match v {
            SerdeJsonValue::Null => Ok(Value::Null),
            SerdeJsonValue::Bool(b) => Ok(Value::Bit(b)),
            SerdeJsonValue::String(s) => Ok(Value::Text(s)),
            SerdeJsonValue::Number(n) => match n.as_f64() {
                Some(n) => Ok(Value::Number(n)),
                _ => Err("not convertible to f64"),
            },
            SerdeJsonValue::Object(_) => Err("cannot convert json object into Value"),
            SerdeJsonValue::Array(_) => Err("cannot convert array into Value"),
        }
    }
}

impl From<Value> for SerdeJsonValue {
    fn from(v: Value) -> SerdeJsonValue {
        match v {
            Value::Null => SerdeJsonValue::Null,
            Value::Text(t) => SerdeJsonValue::String(t),
            Value::Bit(b) => SerdeJsonValue::Bool(b),
            Value::Number(n) => json!(n),
            Value::Link(l) => json!({ "#": l }),
        }
    }
}

impl From<usize> for Value {
    fn from(n: usize) -> Value {
        Value::Number(n as f64)
    }
}

impl From<f32> for Value {
    fn from(n: f32) -> Value {
        Value::Number(n as f64)
    }
}

impl From<u64> for Value {
    fn from(n: u64) -> Value {
        Value::Number(n as f64)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Text(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Text(s)
    }
}
