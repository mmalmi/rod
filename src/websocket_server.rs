// #![deny(warnings)]
use std::collections::{HashMap, HashSet};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::env;

use futures::{SinkExt, StreamExt, TryFutureExt};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};
use warp::Filter;

use serde_json::Value;
use crate::{NetworkAdapter, NetworkAdapterCallback};

/// Our global unique user id counter.
static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

struct User {
    sender: mpsc::UnboundedSender<Message>,
    subscriptions: HashSet<String>
}
impl User {
    fn new(sender: mpsc::UnboundedSender<Message>) -> User {
        User { sender, subscriptions: HashSet::new() }
    }
}

/// Our state of currently connected users.
///
/// - Key is their id
/// - Value is a sender of `warp::ws::Message`
type Users = Arc<RwLock<HashMap<usize, User>>>;

pub struct WebsocketServer {
    on_message_callback: Option<NetworkAdapterCallback>
}

impl NetworkAdapter for WebsocketServer {
    fn on_message(&mut self, callback: NetworkAdapterCallback) {
        self.on_message_callback = Some(callback);
    }

    fn start(&self) {
        self.serve();
    }

    fn stop(&self) {

    }

    fn send_str(&self, m: &String) -> () {

    }
}

impl WebsocketServer {
    pub fn new() -> Self {
        WebsocketServer {
            on_message_callback: None
        }
    }

    #[tokio::main]
    pub async fn serve(&self) {
        pretty_env_logger::init();

        // Keep track of all connected users, key is usize, value
        // is a websocket sender.
        let users = Users::default();
        // Turn our "state" into a new Filter...
        let users = warp::any().map(move || users.clone());

        // GET /gun -> websocket upgrade
        let chat = warp::path("gun")
            // The `ws()` filter will prepare Websocket handshake...
            .and(warp::ws())
            .and(users)
            .map(|ws: warp::ws::Ws, users| {
                // This will call our function if the handshake succeeds.
                ws.on_upgrade(move |socket| Self::user_connected(socket, users))
            });

        let iris = warp::fs::dir("assets/iris");

        let routes = iris.or(chat);

        let port: u16 = match env::var("PORT") {
            Ok(p) => p.parse::<u16>().unwrap(),
            _ => 5000
        };

        eprintln!("Starting server at http://localhost:{}", port);
        warp::serve(routes).run(([0, 0, 0, 0], port)).await;
    }

    async fn user_connected(ws: WebSocket, users: Users) {
        // Use a counter to assign a new unique ID for this user.
        let my_id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);

        eprintln!("new chat user: {}", my_id);

        // Split the socket into a sender and receive of messages.
        let (mut user_ws_tx, mut user_ws_rx) = ws.split();

        // Use a channel to handle buffering and flushing of messages
        // to the websocket...
        let (tx, rx) = mpsc::unbounded_channel();
        let mut rx = UnboundedReceiverStream::new(rx);

        tokio::task::spawn(async move {
            while let Some(message) = rx.next().await {
                let mut errored = false;
                user_ws_tx
                    .send(message)
                    .unwrap_or_else(|e| {
                        eprintln!("websocket send error: {}", e);
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

        // Return a `Future` that is basically a state machine managing
        // this specific user's connection.

        // Every time the user sends a message, broadcast it to
        // all other users...
        while let Some(result) = user_ws_rx.next().await {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    eprintln!("websocket error(uid={}): {}", my_id, e);
                    break;
                }
            };
            Self::user_message(my_id, msg, &users).await;
        }

        // user_ws_rx stream will keep processing as long as the user stays
        // connected. Once they disconnect, then...
        Self::user_disconnected(my_id, &users).await;
    }

    async fn user_message(my_id: usize, msg: Message, users: &Users) {
        let msg_str = if let Ok(s) = msg.to_str() {
            s
        } else {
            return;
        };

        let json: Value = match serde_json::from_str(msg_str) {
            Ok(json) => json,
            Err(_) => { return; }
        };

        println!("{}", json);

        if json.is_array() {
            for sth in json.as_array().iter() {
                for obj in sth.iter() {
                    Self::user_message_single(my_id, users, obj, msg_str).await;
                }
            }
        } else {
            Self::user_message_single(my_id, users, &json, msg_str).await;
        }
    }

    async fn user_message_single(my_id: usize, users: &Users, json: &Value, msg_str: &str) {
        // eprintln!("user {} sent request with id {}, get {} and put {}", my_id, json["#"], json["get"], json["put"]);
        if json["#"] == Value::Null || (json["get"] == Value::Null && json["put"] == Value::Null) {
            // eprintln!("user {} sent funny request {}", my_id, msg_str);
            return;
        }

        if json["get"] != Value::Null {
            match users.write().await.get_mut(&my_id) {
                Some(user) => {
                    match json["get"]["#"].as_str() {
                        Some(path) => { user.subscriptions.insert(path.to_string()); },
                        _ => {}
                    }
                },
                _ => { return; }
            }
        }

        // New message from this user, relay it to everyone else (except same uid)...
        for (&uid, user) in users.read().await.iter() {
            if my_id != uid {
                match &json["put"] {
                    Value::Object(put) => {
                        let mut has = false;
                        for (put_path, _v) in put.into_iter() {
                            for s in user.subscriptions.iter() {
                                if s.find(put_path) != None || put_path.find(s) != None {
                                    has = true;
                                    break;
                                }
                            }
                            if has {
                                break;
                            }
                        }
                        if !has {
                            continue;
                        }
                    },
                    _ => {}
                }
                let _ = user.sender.send(Message::text(json.to_string()));
            }
        }
    }

    async fn user_disconnected(my_id: usize, users: &Users) {
        eprintln!("good bye user: {}", my_id);

        // Stream closed up, so remove from the user list
        users.write().await.remove(&my_id);
    }
}
