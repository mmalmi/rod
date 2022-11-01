use crate::actor::Addr;
use crate::types::{Children, NodeData, Value};
use crate::utils::random_string;
use java_utils::HashCode;
use jsonwebkey as jwk;
use jsonwebtoken::crypto::verify;
use jsonwebtoken::Algorithm;
use log::{debug, error};
use ring::digest::{digest, SHA256};
use serde_json::{json, Value as JsonValue};
use std::collections::{BTreeMap, HashSet};
use std::convert::TryFrom;

#[derive(Clone, Debug)]
pub struct Get {
    pub id: String,
    pub from: Addr,
    pub recipients: Option<HashSet<String>>,
    pub node_id: String,
    pub checksum: Option<i32>,
    pub child_key: Option<String>,
    pub json_str: Option<String>,
}
impl Get {
    pub fn new(node_id: String, child_key: Option<String>, from: Addr) -> Self {
        Self {
            id: random_string(8),
            from,
            recipients: None,
            node_id,
            child_key,
            json_str: None,
            checksum: None,
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
    pub from: Addr,
    pub recipients: Option<HashSet<String>>,
    pub in_response_to: Option<String>,
    pub updated_nodes: BTreeMap<String, Children>,
    pub checksum: Option<i32>,
    pub json_str: Option<String>,
}
impl Put {
    pub fn new(
        updated_nodes: BTreeMap<String, Children>,
        in_response_to: Option<String>,
        from: Addr,
    ) -> Self {
        Self {
            id: random_string(8),
            from,
            recipients: None,
            in_response_to,
            updated_nodes,
            checksum: None,
            json_str: None,
        }
    }

    pub fn new_from_kv(key: String, children: Children, from: Addr) -> Self {
        let mut updated_nodes = BTreeMap::new();
        updated_nodes.insert(key, children);
        Put::new(updated_nodes, None, from)
    }

    pub fn to_string(&mut self) -> String {
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

        let checksum = match &self.checksum {
            Some(s) => *s,
            _ => {
                let put_str = json["put"].to_string();
                let checksum = put_str.hash_code();
                self.checksum = Some(checksum);
                checksum
            }
        };
        json["##"] = json!(checksum);

        let s = json.to_string();
        self.json_str = Some(s.clone());
        s
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    // TODO: NetworkMessage and InternalMessage
    Get(Get),
    Put(Put),
    Hi { from: Addr, peer_id: String },
}

impl Message {
    pub fn to_string(self) -> String {
        match self {
            Message::Get(get) => get.to_string(),
            Message::Put(mut put) => put.to_string(),
            Message::Hi { from: _, peer_id } => json!({"dam": "hi","#": peer_id}).to_string(),
        }
    }

    pub fn get_id(&self) -> String {
        match self {
            Message::Get(get) => get.id.clone(),
            Message::Put(put) => put.id.clone(),
            Message::Hi { from: _, peer_id } => peer_id.to_string(),
        }
    }

    pub fn is_from(&self, addr: &Addr) -> bool {
        match self {
            Message::Get(get) => get.from == *addr,
            Message::Put(put) => put.from == *addr,
            Message::Hi { from, peer_id: _ } => *from == *addr,
        }
    }

    pub fn from(&self) -> Addr {
        match self {
            Message::Get(get) => get.from.clone(),
            Message::Put(put) => put.from.clone(),
            Message::Hi {
                from: _,
                peer_id: _,
            } => Addr::noop(),
        }
    }

    fn verify_sig(
        node_id: &str,
        node_data: &serde_json::Map<String, JsonValue>,
    ) -> Result<(), &'static str> {
        for (child_key, timestamp) in node_data["_"][">"]
            .as_object()
            .ok_or("not an object")?
            .iter()
        {
            let value = node_data
                .get(child_key)
                .ok_or("no matching key in object and _")?;
            let text = value.as_str().ok_or("not a string")?;
            let json: JsonValue =
                serde_json::from_str(text).or(Err("Failed to parse signature as JSON"))?;
            let signature_obj = json.as_object().ok_or("signature json was not an object")?;
            let signed_data = signature_obj
                .get(":")
                .ok_or("no signed data (:) in signature json")?;

            let signed_obj = json!({
                "#": node_id,
                ".": child_key,
                ":": signed_data,
                ">": timestamp
            });

            let signature = signature_obj
                .get("~")
                .ok_or("no signature (~) in signature json")?;
            let signature = signature
                .as_str()
                .ok_or("signature (~) in signature json was not a string")?;
            let signature64 = base64::decode(signature)
                .or(Err("signature (~) in signature json was not base64"))?;
            let signature = base64::encode_config(signature64, base64::URL_SAFE_NO_PAD);
            // TODO use jsonwebtoken underlying ring::signature functions directly, instead of having to re-encode

            let key = &node_id.split("/").next().unwrap()[1..];
            let mut split = key.split(".");
            let x = split.next().unwrap().to_string();
            let y = split
                .next()
                .ok_or("invalid key string: must be in format x.y")?;
            let y = y.to_string();

            let jwk_str = format!("{{\"kty\": \"EC\", \"crv\": \"P-256\", \"x\": \"{}\", \"y\": \"{}\", \"ext\": \"true\"}}", x, y).to_string();
            let my_jwk: jwk::JsonWebKey = jwk_str
                .parse()
                .or(Err("failed to parse JsonWebKey from string"))?;

            let hash = digest(&SHA256, signed_obj.to_string().as_bytes()); // is verify already doing the hashing?

            match verify(
                &signature,
                hash.as_ref(),
                &my_jwk.key.to_decoding_key(),
                Algorithm::ES256,
            ) {
                Ok(is_good) => match is_good {
                    true => continue,
                    _ => return Err("bad signature"),
                },
                Err(_) => {
                    error!(
                        "could not verify signature {} of {}",
                        signature,
                        signed_obj.to_string()
                    );
                    return Err("could not verify signature");
                }
            }
        }
        Ok(())
    }

    fn from_put_obj(
        json: &JsonValue,
        json_str: String,
        msg_id: String,
        from: Addr,
        allow_public_space: bool,
    ) -> Result<Self, &'static str> {
        let obj = json
            .get("put")
            .unwrap()
            .as_object()
            .ok_or("invalid message: msg.put was not an object")?;
        let in_response_to = match json.get("@") {
            Some(in_response_to) => match in_response_to.as_str() {
                Some(in_response_to) => Some(in_response_to.to_string()),
                _ => {
                    return Err("message @ field was not a string");
                }
            },
            _ => None,
        };
        let checksum = match json.get("##") {
            Some(checksum) => match checksum.as_i64() {
                Some(checksum) => Some(checksum as i32),
                _ => None,
            },
            _ => None,
        };
        let mut updated_nodes = BTreeMap::<String, Children>::new();
        for (node_id, node_data) in obj.iter() {
            let node_data = node_data
                .as_object()
                .ok_or("put node data was not an object")?;
            let updated_at_times = node_data["_"][">"] // TODO this panics if _ is not an object and silently crashes the websocket
                .as_object()
                .ok_or("no metadata _ in Put node object")?;

            let mut is_public_space = true;
            if let Some(first_letter) = node_id.chars().next() {
                if first_letter == '~' {
                    // signed data
                    if let Err(e) = Self::verify_sig(node_id, node_data) {
                        error!("invalid sig: {} for msg {}", e, json_str);
                        return Err(e);
                    }
                    is_public_space = false;
                    debug!("valid sig");
                }
            }

            let mut children = Children::default();
            for (child_key, child_val) in node_data.iter() {
                if child_key == "_" {
                    continue;
                }
                let updated_at = updated_at_times
                    .get(child_key)
                    .ok_or("no updated_at found for Put key")?;
                let updated_at = updated_at.as_f64().ok_or("updated_at was not a number")?;
                let value = Value::try_from(child_val.clone())?;

                if node_id == "#" {
                    // content-hash addressed data
                    let content_hash = digest(&SHA256, value.to_string().as_bytes());
                    if *child_key != base64::encode(content_hash.as_ref()) {
                        return Err("invalid content hash");
                    }
                } else if is_public_space && !allow_public_space {
                    return Err("public space writes not allowed (allow_public_space == false)");
                }

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
            json_str: Some(json_str),
        };
        Ok(Message::Put(put))
    }

    fn from_get_obj(
        json: &JsonValue,
        json_str: String,
        msg_id: String,
        from: Addr,
    ) -> Result<Self, &'static str> {
        /* TODO: other types of child_key selectors than equality.

        node.get({'.': {'<': cursor, '-': true}, '%': 20 * 1000}).once().map().on((value, key) => { ...

        '*' wildcard selector

         */

        let get = json.get("get").unwrap();
        let node_id = match get["#"].as_str() {
            Some(str) => str,
            _ => {
                return Err("no node id (#) found in get message");
            }
        };
        let checksum = match json.get("##") {
            Some(checksum) => match checksum.as_i64() {
                Some(checksum) => Some(checksum as i32),
                _ => None,
            },
            _ => None,
        };
        let child_key = match get.get(".") {
            Some(child_key) => match child_key.as_str() {
                Some(child_key) => Some(child_key.to_string()),
                _ => return Err("get child_key . was not a string"),
            },
            _ => None,
        };
        debug!("get node_id {}", node_id);
        let msg_id = msg_id.replace("\"", "");
        let get = Get {
            id: msg_id,
            from,
            recipients: None,
            node_id: node_id.to_string(),
            child_key,
            json_str: Some(json_str),
            checksum,
        };
        Ok(Message::Get(get))
    }

    pub fn from_json_obj(
        json: &JsonValue,
        json_str: String,
        from: Addr,
        allow_public_space: bool,
    ) -> Result<Self, &'static str> {
        let obj = match json.as_object() {
            Some(obj) => obj,
            _ => {
                return Err("not a json object");
            }
        };
        let msg_id = match obj["#"].as_str() {
            Some(str) => str.to_string(),
            _ => {
                return Err("msg id not a string");
            }
        };
        if msg_id.len() > 32 {
            return Err("msg id too long (> 32)");
        }
        if !msg_id.chars().all(char::is_alphanumeric) {
            return Err("msg_id must be alphanumeric");
        }
        if obj.contains_key("put") {
            Self::from_put_obj(json, json_str, msg_id, from, allow_public_space)
        } else if obj.contains_key("get") {
            Self::from_get_obj(json, json_str, msg_id, from)
        } else if let Some(_dam) = obj.get("dam") {
            Ok(Message::Hi {
                from,
                peer_id: msg_id,
            })
        } else {
            Err("Unrecognized message")
        }
    }

    pub fn try_from(s: &str, from: Addr, allow_public_space: bool) -> Result<Vec<Self>, &str> {
        let json: JsonValue = match serde_json::from_str(s) {
            Ok(json) => json,
            Err(_) => {
                return Err("Failed to parse message as JSON");
            }
        };

        if let Some(arr) = json.as_array() {
            let mut vec = Vec::<Self>::new();
            for msg in arr {
                vec.push(Self::from_json_obj(
                    msg,
                    msg.to_string(),
                    from.clone(),
                    allow_public_space,
                )?);
            }
            Ok(vec)
        } else {
            match Self::from_json_obj(&json, s.to_string(), from, allow_public_space) {
                Ok(msg) => Ok(vec![msg]),
                Err(e) => Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::actor::Addr;
    use crate::message::Message;

    #[test]
    fn public_space_write_allowed() {
        Message::try_from(r##"
        [
          {
            "put": {
              "something": {
                "_": {
                  "#": "something",
                  ">": {
                    "else": 1653465227430
                  }
                },
                "else": "{\"sig\":\"aSEA{\\\"m\\\":{\\\"text\\\":\\\"test post\\\",\\\"time\\\":\\\"2022-05-25T07:53:47.424Z\\\",\\\"type\\\":\\\"post\\\",\\\"author\\\":{\\\"keyID\\\":\\\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\\\"}},\\\"s\\\":\\\"WttDQegXyXILtB1nhNq7Jn69MZ0JD/b1LQrIybQ9UuHn86KvKXg9Lg7+ESmeqSQNaQy7KYvfBEEKbd/ClagQOQ==\\\"}\",\"pubKey\":\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\"}"
              }
            },
            "#": "yvd2vk4338i"
          }
        ]
        "##, Addr::noop(), true).unwrap();
    }

    #[test]
    fn public_space_write_disallowed() {
        let res = Message::try_from(
            r##"
        [
          {
            "put": {
              "something": {
                "_": {
                  "#": "something",
                  ">": {
                    "else": 1653465227430
                  }
                },
                "else": "{\"sig\":\"aSEA{\\\"m\\\":{\\\"text\\\":\\\"test post\\\",\\\"time\\\":\\\"2022-05-25T07:53:47.424Z\\\",\\\"type\\\":\\\"post\\\",\\\"author\\\":{\\\"keyID\\\":\\\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\\\"}},\\\"s\\\":\\\"WttDQegXyXILtB1nhNq7Jn69MZ0JD/b1LQrIybQ9UuHn86KvKXg9Lg7+ESmeqSQNaQy7KYvfBEEKbd/ClagQOQ==\\\"}\",\"pubKey\":\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\"}"
              }
            },
            "#": "yvd2vk4338i"
          }
        ]
        "##,
            Addr::noop(),
            false,
        );
        assert!(res.is_err());
    }

    #[test]
    fn valid_content_addressed_data() {
        Message::try_from(r##"
        [
          {
            "put": {
              "#": {
                "_": {
                  "#": "#",
                  ">": {
                    "rkHfUdMssQ8Ln9LtiuPTb/ntNxR6HZiVdVsn9DdnKZs=": 1653465227430
                  }
                },
                "rkHfUdMssQ8Ln9LtiuPTb/ntNxR6HZiVdVsn9DdnKZs=": "{\"sig\":\"aSEA{\\\"m\\\":{\\\"text\\\":\\\"test post\\\",\\\"time\\\":\\\"2022-05-25T07:53:47.424Z\\\",\\\"type\\\":\\\"post\\\",\\\"author\\\":{\\\"keyID\\\":\\\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\\\"}},\\\"s\\\":\\\"WttDQegXyXILtB1nhNq7Jn69MZ0JD/b1LQrIybQ9UuHn86KvKXg9Lg7+ESmeqSQNaQy7KYvfBEEKbd/ClagQOQ==\\\"}\",\"pubKey\":\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\"}"
              }
            },
            "#": "yvd2vk4338i"
          }
        ]
        "##, Addr::noop(), false).unwrap();
    }

    #[test]
    fn invalid_content_addressed_data() {
        let res = Message::try_from(
            r##"
        [
          {
            "put": {
              "#": {
                "_": {
                  "#": "#",
                  ">": {
                    "rkHfUdMssQ8Ln9LtiuPTb/ntNxR6HZiVdVsn9DdnKZs=": 1653465227430
                  }
                },
                "rkHfUdMssQ8Ln9LtiuPTb/ntNxR6HZiVdVsn9DdnKZs=": "{\"sig\":\"aSEA{\\\"m\\\":{\\\"text\\\":\\\"invalid test post\\\",\\\"time\\\":\\\"2022-05-25T07:53:47.424Z\\\",\\\"type\\\":\\\"post\\\",\\\"author\\\":{\\\"keyID\\\":\\\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\\\"}},\\\"s\\\":\\\"WttDQegXyXILtB1nhNq7Jn69MZ0JD/b1LQrIybQ9UuHn86KvKXg9Lg7+ESmeqSQNaQy7KYvfBEEKbd/ClagQOQ==\\\"}\",\"pubKey\":\"U2CjHOxXiF7Giyjr_V5Mb2VoyWnRJCyFqEuwObn3pdM.UtCpoyYTG7JJTitZVJhSpxXtD0eHE45iT2Zj--P_n-U\"}"
              }
            },
            "#": "yvd2vk4338i"
          }
        ]
        "##,
            Addr::noop(),
            false,
        );
        assert!(res.is_err());
    }

    #[test]
    fn valid_user_signed_data() {
        Message::try_from(r##"
        {
          "put": {
            "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8": {
              "_": {
                "#": "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8",
                ">": {
                  "profile": 1653463165115
                }
              },
              "profile": "{\":\":{\"#\":\"~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile\"},\"~\":\"JW+tFHHVBaY+zm/uzUoGVlogvXXQIA3vFNT0f0uX6tnnPGrRevDWzEmnVYy+ChxS6AJi5THiPyOc2HorIIM5wg==\"}"
            },
            "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile": {
              "_": {
                ">": {
                  "name": 1653463165115
                },
                "#": "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile"
              },
              "name": "{\":\":\"Arja Koriseva\",\"~\":\"KCq2D/T0mMenizxiVMso8FO5JIv9ZJLA0Q67DFa9qssPSKCmmieC1Nl5+nRpOX29C6A2/kLaJgphN/X7kUQjww==\"}"
            }
          },
          "#": "issWkzotF"
        }
        "##, Addr::noop(), false).unwrap();
    }

    #[test]
    fn invalid_user_signed_data() {
        let res = Message::try_from(
            r##"
        {
          "put": {
            "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8": {
              "_": {
                "#": "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8",
                ">": {
                  "profile": 1653463165115
                }
              },
              "profile": "{\":\":{\"#\":\"~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile\"},\"~\":\"JW+tFHHVBaY+zm/uzUoGVlogvXXQIA3vFNT0f0uX6tnnPGrRevDWzEmnVYy+ChxS6AJi5THiPyOc2HorIIM5wg==\"}"
            },
            "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile": {
              "_": {
                ">": {
                  "name": 1653463165115
                },
                "#": "~BjxYTmcODm__M52FmMX_grHcafW0WiHpJUtVRCgEsZY._QiIs4tK22hebiZjGovtp3cHo1pAfYxoRODS_jyudA8/profile"
              },
              "name": "{\":\":\"Fake Arja Koriseva\",\"~\":\"KCq2D/T0mMenizxiVMso8FO5JIv9ZJLA0Q67DFa9qssPSKCmmieC1Nl5+nRpOX29C6A2/kLaJgphN/X7kUQjww==\"}"
            }
          },
          "#": "issWkzotF"
        }
        "##,
            Addr::noop(),
            false,
        );
        assert!(res.is_err());
    }
}
