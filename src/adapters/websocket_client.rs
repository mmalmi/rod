use futures_util::{SinkExt, StreamExt};
use log::*;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error, Result},
    WebSocketStream
};
use tokio::sync::{mpsc, RwLock};
use url::Url;
use std::collections::HashMap;

use crate::types::NetworkAdapter;
use crate::Node;
use async_trait::async_trait;
use log::{debug};
use serde_json::Value;
use std::sync::Arc;

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
    sockets: Sockets
}

#[async_trait]
impl NetworkAdapter for WebsocketClient {
    fn new(node: Node) -> Self {
        WebsocketClient {
            node: node.clone(),
            sockets: Sockets::default()
        }
    }

    async fn start(&self) {
        debug!("starting WebsocketClient\n");

        let (mut socket, _) = connect_async(
            Url::parse("wss://gun-us.herokuapp.com/gun").expect("Can't connect to URL"),
        ).await.unwrap();
        debug!("connected");
        self.sockets.write().await.insert("sth".to_string(), socket);
        listen(self.node.clone(), self.sockets.clone());
    }

    fn stop(&self) {

    }

    fn send_str(&self, m: &String) -> () {

    }
}

    async fn user_connected(mut node: Node, ws: WebSocket, users: Users) { // TODO copied from server, need similar here.
        // Use a counter to assign a new unique ID for this user.
        let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

        debug!("new chat user: {}", my_id);

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
                let mut errored = false;
                user_ws_tx
                    .send(message)
                    .unwrap_or_else(|e| {
                        debug!("websocket send error: {}", e);
                        errored = true;
                    })
                    .await;
                if errored {
                    let _ = user_ws_tx.close().await;
                    break;
                }
            }
        });

        // Save the sender in our list of connected users.
        let user = User::new(tx);
        users.write().await.insert(my_id, user);

        node.get("node_stats").get("connection_count").put(users.read().await.len().to_string().into());

        // Return a `Future` that is basically a state machine managing
        // this specific user's connection.

        // Every time the user sends a message, broadcast it to
        // all other users...
        while let Some(result) = user_ws_rx.next().await {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    debug!("websocket error(uid={}): {}", my_id, e);
                    break;
                }
            };
            Self::user_message(&mut node, my_id, msg, &users).await;
        }

        // user_ws_rx stream will keep processing as long as the user stays
        // connected. Once they disconnect, then...
        Self::user_disconnected(my_id, &users).await;
        node.get("node_stats").get("connection_count").put(users.read().await.len().to_string().into());
    }

async fn listen(node: Node, sockets: Sockets) {
    for socket in sockets.read().await.values() {
        while let Some(msg) = socket.next().await {
            let msg = msg.unwrap();
            if msg.is_text() {
                debug!("msg {}", msg);
                let json: Value = match serde_json::from_str(&msg.into_text().unwrap()) {
                    Ok(json) => json,
                    Err(_) => { continue; }
                };
                node.clone().incoming_message(&json, false);
            }
        }
    };
}

const AGENT: &str = "Tungstenite";

async fn get_case_count() -> Result<u32> {
    let (mut socket, _) = connect_async(
        Url::parse("ws://localhost:9001/getCaseCount").expect("Can't connect to case count URL"),
    )
    .await?;
    let msg = socket.next().await.expect("Can't fetch case count")?;
    socket.close(None).await?;
    Ok(msg.into_text()?.parse::<u32>().expect("Can't parse case count"))
}

async fn update_reports() -> Result<()> {
    let (mut socket, _) = connect_async(
        Url::parse(&format!("ws://localhost:9001/updateReports?agent={}", AGENT))
            .expect("Can't update reports"),
    )
    .await?;
    socket.close(None).await?;
    Ok(())
}

async fn run_test(case: u32) -> Result<()> {
    info!("Running test case {}", case);
    let case_url =
        Url::parse(&format!("ws://localhost:9001/runCase?case={}&agent={}", case, AGENT))
            .expect("Bad testcase URL");

    let (mut ws_stream, _) = connect_async(case_url).await?;
    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            ws_stream.send(msg).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let total = get_case_count().await.expect("Error getting case count");

    for case in 1..=total {
        if let Err(e) = run_test(case).await {
            match e {
                Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                err => error!("Testcase failed: {}", err),
            }
        }
    }

    update_reports().await.expect("Error updating reports");
}