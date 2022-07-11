use crate::actor::{Actor, ActorContext};
use crate::message::Message;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;

use async_trait::async_trait;

use futures_util::{future, TryStreamExt};
use log::{debug, error, info};

use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type WsSender = SplitSink<WsStream, WsMessage>;
type WsReceiver = SplitStream<WsStream>;

pub struct WsConn {
    sender: WsSender,
    receiver: Option<WsReceiver>,
    allow_public_space: bool,
}

impl WsConn {
    pub fn new(sender: WsSender, receiver: WsReceiver, allow_public_space: bool) -> Self {
        Self {
            sender: sender,
            receiver: Some(receiver),
            allow_public_space,
        }
    }
}

#[async_trait]
impl Actor for WsConn {
    async fn handle(&mut self, msg: Message, _ctx: &ActorContext) {
        let _ = self.sender.send(WsMessage::Text(msg.to_string())).await;
    }

    async fn pre_start(&mut self, ctx: &ActorContext) {
        info!("WsConn starting");
        let hi = Message::Hi {
            from: ctx.addr.clone(),
            peer_id: ctx.peer_id.read().unwrap().clone(),
        };
        let _ = self.sender.send(WsMessage::Text(hi.to_string())).await;
        let receiver = self.receiver.take().unwrap();
        let mut ctx2 = ctx.clone();
        let allow_public_space = self.allow_public_space;
        ctx.child_task(async move {
            let _ = receiver
                .try_for_each(|msg| {
                    if let Ok(s) = msg.to_text() {
                        match Message::try_from(s, ctx2.addr.clone(), allow_public_space) {
                            Ok(msgs) => {
                                debug!("ws_conn in {}", s);
                                for msg in msgs.into_iter() {
                                    if ctx2.router.send(msg).is_err() {
                                        error!("failed to send incoming message to node");
                                    }
                                }
                            }
                            _ => {}
                        };
                    }
                    future::ok(())
                })
                .await;
            ctx2.stop();
        });
    }

    async fn stopping(&mut self, _context: &ActorContext) {
        info!("WsConn stopping");
    }
}
