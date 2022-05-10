use futures_util::{StreamExt, SinkExt};
use futures::stream::{SplitStream, SplitSink};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message as WsMessage},
    WebSocketStream
};
use url::Url;
use std::collections::HashSet;
use std::sync::Arc;

use crate::message::Message;
use crate::actor::{Actor, Addr, ActorContext, start_actor};
use crate::Config;
use async_trait::async_trait;
use log::{debug, error, info};
use tokio::time::{sleep, Duration};

pub struct OutgoingWebsocketManager {
    clients: HashSet<Arc<Addr>>,
    config: Config
}

impl OutgoingWebsocketManager {
    pub fn new(config: Config) -> Self {
        OutgoingWebsocketManager {
            config,
            clients: HashSet::new()
        }
    }
}

#[async_trait]
impl Actor for OutgoingWebsocketManager { // TODO: support multiple outbound websockets
    async fn started(&mut self, ctx: &ActorContext) {
        info!("OutgoingWebsocketManager starting");
        for url in self.config.outgoing_websocket_peers.iter() {
            let url = url.to_string();
            loop { // TODO break on actor shutdown
                let result = connect_async(
                    Url::parse(&url).expect("Can't connect to URL"),
                ).await;
                if let Ok(tuple) = result {
                    let (socket, _) = tuple;
                    debug!("outgoing websocket opened to {}", url);
                    let client = WebsocketClient::new(socket);
                    let addr = start_actor(Box::new(client), ctx);
                    self.clients.insert(addr);
                }
                sleep(Duration::from_millis(1000)).await;
            }
        }
    }

    async fn handle(&mut self, message: Message, _ctx: &ActorContext) {
        for client in self.clients.iter() {
            client.sender.try_send(message.clone());
        }
    }
}

type WsStream = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct WebsocketClient {
    sender: SplitSink<WsStream, tokio_tungstenite::tungstenite::Message>,
    receiver: SplitStream<WsStream>
}

impl WebsocketClient {
    pub fn new(socket: WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>) -> Self {
        let (sender, receiver) = socket.split();
        WebsocketClient {
            sender,
            receiver
        }
    }
}

#[async_trait]
impl Actor for WebsocketClient {
    async fn started(&mut self, ctx: &ActorContext) {
        // Split the socket into a sender and receive of messages.

        // Return a `Future` that is basically a state machine managing
        // this specific user's connection.

        // Every time the user sends a message, broadcast it to
        // all other users...
        while let Some(result) = self.receiver.next().await {
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
                    match Message::try_from(s, (*ctx.addr.upgrade().unwrap()).clone()) {
                        Ok(msgs) => {
                            for msg in msgs.into_iter() {
                                if let Err(e) = ctx.router.sender.try_send(msg) {
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

    async fn handle(&mut self, message: Message, _ctx: &ActorContext) {
        if let Err(_) = self.sender.send(WsMessage::text(message.to_string())).await {
            // TODO stop actor
        }
    }
}
