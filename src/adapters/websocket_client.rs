use futures_util::{SinkExt, StreamExt};
use std::env;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message},
    WebSocketStream
};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
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
    sender: mpsc::Sender<Message>
}
impl User {
    fn new(sender: mpsc::Sender<Message>) -> User {
        User { sender }
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

    fn send_str(&self, m: &String, from: &String) -> () {
        let users = self.users.clone();
        let m = m.clone();
        let from = from.clone();
        tokio::task::spawn(async move { // TODO instead, send a message to a sender task via bounded channel
            for (id, user) in users.read().await.iter() {
                if id == &from {
                    continue;
                }
                //debug!("WS CLIENT SEND\n");
                let _ = user.sender.try_send(Message::text(m.to_string()));
            }
        });
    }
}

async fn user_connected(mut node: Node, ws: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, users: Users) { // TODO copied from server, need similar here.
    let my_id = "wss://gun-us.herokuapp.com/gun".to_string();

    debug!("outgoing websocket opened: {}", my_id);

    // Split the socket into a sender and receive of messages.
    let (mut user_ws_tx, mut user_ws_rx) = ws.split();

    // Use a channel to handle buffering and flushing of messages
    // to the websocket...

    let channel_size: u16 = match env::var("RUST_CHANNEL_SIZE") {
        Ok(p) => p.parse::<u16>().unwrap(),
        _ => 10
    };

    let (tx, rx) = mpsc::channel(channel_size.into());
    let mut rx = ReceiverStream::new(rx);

    tokio::task::spawn(async move {
        while let Some(message) = rx.next().await {
            user_ws_tx.send(message).await;
        }
    });

    // Save the sender in our list of connected users.
    let user = User::new(tx);
    users.write().await.insert(my_id.clone(), user);

    node.get("node_stats").get("websocket_client_connections").put(users.read().await.len().to_string().into());

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
    node.get("node_stats").get("websocket_client_connections").put(users.read().await.len().to_string().into());
}

