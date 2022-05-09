use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as WsMessage},
    WebSocketStream
};
use url::Url;
use std::collections::HashMap;

use crate::message::Message;
use crate::actor::{Actor, Addr};
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use tokio::time::{sleep, Duration};
use tokio::sync::mpsc::{Receiver, channel};

pub struct OutgoingWebsocketManager {
    node: Node,
    receiver: Receiver<Message>,
    clients: HashMap<String, Addr>
}

#[async_trait]
impl Actor for OutgoingWebsocketManager { // TODO: support multiple outbound websockets
    fn new(receiver: Receiver<Message>, node: Node) -> Self {
        OutgoingWebsocketManager {
            node,
            receiver,
            clients: HashMap::new()
        }
    }

    async fn start(&self) {
        let config = self.node.config.read().unwrap().clone();
        for peer in config.outgoing_websocket_peers {
            loop { // TODO don't reconnect if self.receiver is closed
                let result = connect_async(
                    Url::parse(&peer).expect("Can't connect to URL"),
                ).await;
                if let Ok(tuple) = result {
                    let (socket, _) = tuple;
                    debug!("outgoing websocket opened to {}", peer.to_string());
                    let (sender, receiver) = channel::<Message>(config.rust_channel_size);
                    let client = WebsocketClient::new_with_socket(socket, receiver, self.node.clone());
                    self.clients.insert(peer.to_string(), Addr::new(sender));
                    tokio::spawn(async move { client.start().await });
                }
                sleep(Duration::from_millis(1000)).await;
            }
        }
    }
}

pub struct WebsocketClient {
    node: Node,
    receiver: Receiver<Message>,
    socket: Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>
}

#[async_trait]
impl Actor for WebsocketClient { // TODO: support multiple outbound websockets
    fn new(receiver: Receiver<Message>, node: Node) -> Self {
        WebsocketClient {
            node,
            receiver,
            socket: None // uh oh, what to do with custom constructors
        }
    }

    async fn start(&self) {
        let ws = match &self.socket {
            Some(s) => s,
            _ => { return; }
        };

        // Split the socket into a sender and receive of messages.
        let (mut user_ws_tx, mut user_ws_rx) = ws.split();

        tokio::task::spawn(async move {
            while let Some(message) = self.receiver.recv().await {
                if let Err(_) = user_ws_tx.send(WsMessage::text(message.to_string())).await {
                    break;
                }
            }
        });

        // Return a `Future` that is basically a state machine managing
        // this specific user's connection.

        // Every time the user sends a message, broadcast it to
        // all other users...
        let router = self.node.get_router_addr().unwrap();
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
                    match Message::try_from(s, self.addr.clone()) {
                        Ok(msgs) => {
                            for msg in msgs.into_iter() {
                                if let Err(e) = router.sender.try_send(msg) {
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
    }
}

impl WebsocketClient {
    pub fn new_with_socket(socket: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, receiver: Receiver<Message>, node: Node) -> Self {
        Self {
            socket: Some(socket),
            receiver,
            node
        }
    }
}

