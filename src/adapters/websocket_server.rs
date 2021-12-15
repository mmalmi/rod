// #![deny(warnings)]
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::env;

use futures_util::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::RwLock;
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

}
impl User {
    fn new() -> User {
        User { }
    }
}

type Users = Arc<RwLock<HashMap<String, User>>>;

pub struct WebsocketServer {
    node: Node,
    users: Users
}

#[async_trait]
impl NetworkAdapter for WebsocketServer {
    fn new(node: Node) -> Self {
        WebsocketServer {
            node,
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
}

impl WebsocketServer {
    async fn serve(node: Node, users: Users) {
        // Turn our "state" into a new Filter...
        let users = warp::any().map(move || users.clone());

        let peer_id = node.get_peer_id();
        //let peer_id_route = warp::path!("peer_id").map(|| peer_id);
        let peer_id_route = warp::path!("peer_id").map(move || format!("{}", peer_id));

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

        let routes = iris.or(assets).or(chat).or(peer_id_route);

        let port: u16 = match env::var("PORT") {
            Ok(p) => p.parse::<u16>().unwrap(),
            _ => 4944
        };

        if let Ok(cert_path) = env::var("CERT_PATH") {
            if let Ok(key_path) = env::var("KEY_PATH") {
                return warp::serve(routes)
                    .tls()
                    .cert_path(cert_path)
                    .key_path(key_path)
                    .run(([0, 0, 0, 0], port)).await;
            }
        }

        warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    }

    async fn user_connected(mut node: Node, ws: WebSocket, users: Users) {
        // Use a counter to assign a new unique ID for this user.
        let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);
        let my_id = format!("ws_server_{}", my_id).to_string();

        debug!("new chat user: {}", my_id);

        // Split the socket into a sender and receive of messages.
        let (mut user_ws_tx, mut user_ws_rx) = ws.split();

        let mut rx = node.get_outgoing_msg_receiver();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await {
                    if let Err(_) = user_ws_tx.send(Message::text(message.msg)).await {
                        break;
                    }
                }
            }
        });

        let user = User::new();
        users.write().await.insert(my_id.clone(), user);

        let peer_id = node.get_peer_id();
        node.get("node_stats").get(&peer_id).get("websocket_server_connections").put(users.read().await.len().to_string().into());

        // Pass incoming messages to the Node
        while let Some(result) = user_ws_rx.next().await {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    debug!("websocket error(uid={}): {}", my_id, e);
                    break;
                }
            };
            if let Ok(s) = msg.to_str() {
                node.incoming_message(s.to_string(), &my_id);
            }
        }

        Self::user_disconnected(my_id, &users).await;
        node.get("node_stats").get(&peer_id).get("websocket_server_connections").put(users.read().await.len().to_string().into());
    }

    async fn user_disconnected(my_id: String, users: &Users) {
        debug!("good bye user: {}", my_id);

        // Stream closed up, so remove from the user list
        users.write().await.remove(&my_id);
    }
}
