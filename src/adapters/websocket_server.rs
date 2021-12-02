// #![deny(warnings)]
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::env;

use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use async_trait::async_trait;

use crate::types::NetworkAdapter;
use crate::Node;
use log::{debug};

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

struct User {
    sender: mpsc::Sender<Message>
}
impl User {
    fn new(sender: mpsc::Sender<Message>) -> User {
        User { sender }
    }
}

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, User>>>;

pub struct WebsocketServer {
    node: Node,
    users: Users
}

#[async_trait]
impl NetworkAdapter for WebsocketServer {
    fn new(node: Node) -> Self {
        WebsocketServer {
            node: node.clone(),
            users: Users::default()
        }
    }

    async fn start(&self) {
        let node = self.node.clone();
        let users = self.users.clone();
        Self::serve(node, users).await;
    }

    fn stop(&self) {

    }

    fn send_str(&self, m: &String) -> () {
        let users = self.users.clone();
        let m = m.clone();
        tokio::task::spawn(async move {
            Self::send_str(users, &m).await;
        });
    }
}

impl WebsocketServer {
    async fn send_str(users: Users, m: &String) {
        for user in users.read().await.values() {
            debug!("out {}\n", m);
            let _ = user.sender.try_send(Message::text(m));
        }
    }

    async fn serve(node: Node, users: Users) {
        // Turn our "state" into a new Filter...
        let users = warp::any().map(move || users.clone());

        // GET /gun -> websocket upgrade
        let chat = warp::path("gun")
            // The `ws()` filter will prepare Websocket handshake...
            .and(warp::ws())
            .and(users)
            .map(move |ws: warp::ws::Ws, users| {
                // This will call our function if the handshake succeeds.
                let node_clone = node.clone();
                ws.on_upgrade(move |socket| Self::user_connected(node_clone.clone(), socket, users))
            });

        let iris = warp::fs::dir("assets/iris");
        let assets = warp::fs::dir("assets");

        let routes = iris.or(assets).or(chat);

        let port: u16 = match env::var("PORT") {
            Ok(p) => p.parse::<u16>().unwrap(),
            _ => 5000
        };

        debug!("Starting server at http://localhost:{}", port);
        warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    }

    async fn user_connected(mut node: Node, ws: WebSocket, users: Users) {
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

        node.get("node_stats").get("websocket_server_connections").put(users.read().await.len().to_string().into());

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
            if let Ok(s) = msg.to_str() {
                node.incoming_message(s.to_string());
            }
        }

        // user_ws_rx stream will keep processing as long as the user stays
        // connected. Once they disconnect, then...
        Self::user_disconnected(my_id, &users).await;
        node.get("node_stats").get("websocket_server_connections").put(users.read().await.len().to_string().into());
    }

    async fn user_disconnected(my_id: usize, users: &Users) {
        debug!("good bye user: {}", my_id); // TODO there are often many connections started per client but not closed

        // Stream closed up, so remove from the user list
        users.write().await.remove(&my_id);
    }
}
