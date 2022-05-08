use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as WsMessage},
    WebSocketStream
};
use tokio::sync::RwLock;
use url::Url;
use std::collections::HashMap;

use crate::message::Message;
use crate::actor::Actor;
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use std::sync::Arc;
use tokio::time::{sleep, Duration};

type Users = Arc<RwLock<HashMap<String, User>>>;

struct User {

}
impl User {
    fn new() -> User {
        User { }
    }
}

pub struct WebsocketClient {
    node: Node,
    users: Users
}

#[async_trait]
impl Actor for WebsocketClient {
    fn new(node: Node) -> Self {
        WebsocketClient {
            node,
            users: Users::default()
        }
    }

    async fn start(&self) {
        let config = self.node.config.read().unwrap().clone();
        for peer in config.outgoing_websocket_peers {
            let node = self.node.clone();
            let users = self.users.clone();
            tokio::task::spawn(async move {
                debug!("WebsocketClient connecting to {}\n", peer);
                loop {
                    let node = node.clone();
                    let users = users.clone();
                    let result = connect_async(
                        Url::parse(&peer).expect("Can't connect to URL"),
                    ).await;
                    if let Ok(tuple) = result {
                        let (socket, _) = tuple;
                        debug!("connected");
                        user_connected(node, socket, users).await;
                    }
                    sleep(Duration::from_millis(1000)).await;
                }
            });
        }
    }
}

async fn user_connected(mut node: Node, ws: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, users: Users) { // TODO copied from server, need similar here.
    let my_id = "wss://gun-us.herokuapp.com/gun".to_string();

    debug!("outgoing websocket opened: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    let mut rx = node.get_outgoing_msg_receiver();

    let update_stats = node.config.read().unwrap().stats;

    let my_id_clone = my_id.clone();
    tokio::task::spawn(async move { // TODO as in websocket_server, there should be only 1 task that relays to the addressed message recipient
        loop {
            if let Ok(message) = rx.recv().await {
                let from = match message.clone() {
                    Message::Get(get) => get.from,
                    Message::Put(put) => put.from,
                    _ => "".to_string()
                };
                if from == my_id_clone {
                    continue;
                }
                if let Err(_) = user_ws_tx.send(WsMessage::text(message.to_string())).await {
                    break;
                }
            }
        }
    });

    // Save the sender in our list of connected users.
    let user = User::new();
    users.write().await.insert(my_id.clone(), user);

    let peer_id = node.get_peer_id();
    if update_stats {
        node.get("node_stats").get(&peer_id).get("websocket_client_connections").put(users.read().await.len().to_string().into());
    }

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    // Every time the user sends a message, broadcast it to
    // all other users...
    let incoming_message_sender = node.get_incoming_msg_sender();
    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                error!("websocket receive error: {}", e);
                break;
            }
        };
        match msg.to_text() {
            Ok(s) => {
                if s == "PING" {
                    continue;
                }
                match Message::try_from(s, my_id.clone()) {
                    Ok(msgs) => {
                        for msg in msgs.into_iter() {
                            if let Err(e) = incoming_message_sender.try_send(msg) {
                                error!("failed to send incoming message to node: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        error!("{}", e);
                        continue;
                    }
                }
            },
            Err(e) => {
                error!("websocket incoming msg .to_text() failed: {}", e);
                break;
            }
        };
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    users.write().await.remove(&my_id);
    if update_stats {
        node.get("node_stats").get(&peer_id).get("websocket_client_connections").put(users.read().await.len().to_string().into());
    }
}

