use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as WsMessage},
    WebSocketStream
};
use url::Url;
use std::collections::HashSet;

use crate::message::Message;
use crate::actor::{Actor, Addr};
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use tokio::time::{sleep, Duration};
use tokio::sync::mpsc::{Receiver, channel};

pub struct OutgoingWebsocketManager {
    clients: HashSet<Addr>
}

impl OutgoingWebsocketManager {
    fn new() -> Self {
        OutgoingWebsocketManager {
            clients: HashSet::new()
        }
    }
}

#[async_trait]
impl Actor for OutgoingWebsocketManager { // TODO: support multiple outbound websockets
    async fn started(&self) {
        let config = self.node.config.read().unwrap().clone();
        for peer in config.outgoing_websocket_peers {
            loop { // TODO don't reconnect if self.receiver is closed
                let result = connect_async(
                    Url::parse(&peer).expect("Can't connect to URL"),
                ).await;
                if let Ok(tuple) = result {
                    let (socket, _) = tuple;
                    debug!("outgoing websocket opened to {}", peer.to_string());
                    let client = WebsocketClient::new_with_socket(socket, receiver);
                    let addr = start_actor(client).await;
                    self.clients.insert(addr);
                }
                sleep(Duration::from_millis(1000)).await;
            }
        }
    }

    async fn handle(&self, message: Message) {
        for client in self.clients {
            client.sender.try_send(message.clone());
        }
    }
}

pub struct WebsocketClient {
    socket: Option<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>
}

#[async_trait]
impl Actor for WebsocketClient {
    async fn started(&self) {
        let ws = match &self.socket {
            Some(s) => s,
            _ => { return; } // TODO stop
        };

        // Split the socket into a sender and receive of messages.
        let (mut user_ws_tx, mut user_ws_rx) = ws.split();

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

    fn handle(&self, message: Message) {
        if let Err(_) = user_ws_tx.send(WsMessage::text(message.to_string())).await {
            // TODO stop actor
        }
    }
}

impl WebsocketClient {
    fn new(socket: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> Self {
        WebsocketClient {
            socket
        }
    }
}

