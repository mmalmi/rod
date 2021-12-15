use futures_util::{SinkExt, StreamExt};
use std::env;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message},
    WebSocketStream
};
use tokio::sync::RwLock;
use url::Url;
use std::collections::HashMap;

use crate::types::NetworkAdapter;
use crate::Node;
use async_trait::async_trait;
use log::{debug};
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
impl NetworkAdapter for WebsocketClient {
    fn new(node: Node) -> Self {
        WebsocketClient {
            node,
            users: Users::default()
        }
    }

    async fn start(&self) { // "wss://gun-us.herokuapp.com/gun"
        if let Ok(peers) = env::var("PEERS") {
            debug!("WebsocketClient connecting to {}\n", peers);
            loop {
                let result = connect_async(
                    Url::parse(&peers).expect("Can't connect to URL"),
                ).await;
                if let Ok(tuple) = result {
                    let (socket, _) = tuple;
                    debug!("connected");
                    user_connected(self.node.clone(), socket, self.users.clone()).await;
                }
                sleep(Duration::from_millis(1000)).await;
            }
        }
    }

    fn stop(&self) {

    }
}

async fn user_connected(mut node: Node, ws: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, users: Users) { // TODO copied from server, need similar here.
    let my_id = "wss://gun-us.herokuapp.com/gun".to_string();

    debug!("outgoing websocket opened: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    let mut rx = node.get_outgoing_msg_receiver();

    tokio::task::spawn(async move {
        while let Ok(message) = rx.recv().await {
            user_ws_tx.send(Message::text(message.msg)).await;
        }
    });

    // Save the sender in our list of connected users.
    let user = User::new();
    users.write().await.insert(my_id.clone(), user);

    let peer_id = node.get_peer_id();
    node.get("node_stats").get(&peer_id).get("websocket_client_connections").put(users.read().await.len().to_string().into());

    // Return a `Future` that is basically a state machine managing
    // this specific user's connection.

    // Every time the user sends a message, broadcast it to
    // all other users...
    while let Some(result) = user_ws_rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                break;
            }
        };
        if let Ok(s) = msg.to_text() {
            node.incoming_message(s.to_string(), &my_id);
        }
    }

    // user_ws_rx stream will keep processing as long as the user stays
    // connected. Once they disconnect, then...
    users.write().await.remove(&my_id);
    node.get("node_stats").get(&peer_id).get("websocket_client_connections").put(users.read().await.len().to_string().into());
}

