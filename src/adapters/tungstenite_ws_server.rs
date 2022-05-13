use crate::message::Message;
use crate::actor::{Actor, Addr, ActorContext};
use crate::Config;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::{Arc};
use tokio::sync::RwLock;

use futures_util::{future, StreamExt, TryStreamExt};
use log::{info, error, debug};
use tokio::net::{TcpListener, TcpStream};

use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::{Message as WsMessage};

type WsSender = SplitSink<WebSocketStream<tokio::net::TcpStream>, WsMessage>;
type WsReceiver = SplitStream<WebSocketStream<tokio::net::TcpStream>>;
type Clients = Arc<RwLock<HashSet<Addr>>>;

pub struct WsServer2 {
    config: Config,
    clients: Clients
}
impl WsServer2 {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            clients: Clients::default()
        }
    }
}
#[async_trait]
impl Actor for WsServer2 {
    async fn handle(&mut self, msg: Message, ctx: &ActorContext) {
        for conn in self.clients.read().await.iter() {
            let _ = conn.sender.send(msg.clone());
        }
    }

    async fn pre_start(&mut self, ctx: &ActorContext) {
        let addr = format!("0.0.0.0:{}", self.config.websocket_server_port).to_string();

        // Create the event loop and TCP listener we'll accept connections on.
        let try_socket = TcpListener::bind(&addr).await;
        let listener = try_socket.expect("Failed to bind");
        info!("Listening on: {}", addr);

        let clients = self.clients.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let addr = stream.peer_addr().expect("connected streams should have a peer address");
                info!("Peer address: {}", addr);

                let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                    Ok(s) => s,
                    Err(e) => {
                        // suppress errors from receiving normal http requests
                        // error!("Error during the websocket handshake occurred: {}", e);
                        continue;
                    }
                };

                info!("New WebSocket connection: {}", addr);
                let (sender, receiver) = ws_stream.split();

                let conn = WsConn::new(sender, receiver);
                let addr = ctx.start_actor(Box::new(conn));
                clients.write().await.insert(addr);
            }
        });
    }
}

struct WsConn {
    sender: WsSender,
    receiver: Option<WsReceiver>
}

impl WsConn {
    pub fn new(sender: WsSender, receiver: WsReceiver) -> Self {
        Self {
            sender: sender,
            receiver: Some(receiver)
        }
    }
}

#[async_trait]
impl Actor for WsConn {
    async fn handle(&mut self, msg: Message, ctx: &ActorContext) {
        let _ = self.sender.send(WsMessage::Text(msg.to_string())).await;
    }

    async fn pre_start(&mut self, ctx: &ActorContext) {
        let receiver = self.receiver.take().unwrap();
        let ctx2 = ctx.clone();
        ctx.abort_on_stop(tokio::spawn(async move {
            receiver.try_for_each(|msg| {
                if let Ok(s) = msg.to_text() {
                    match Message::try_from(s, ctx2.addr.clone()) {
                        Ok(msgs) => {
                            debug!("websocket_client in");
                            for msg in msgs.into_iter() {
                                if let Err(e) = ctx2.router.sender.send(msg) {
                                    error!("failed to send incoming message to node: {}", e);
                                }
                            }
                        },
                        Err(e) => {
                            //error!("{}", e);
                        }
                    };
                }
                future::ok(())
            }).await;
        }));
    }
}

async fn accept_connection(stream: TcpStream) {

}