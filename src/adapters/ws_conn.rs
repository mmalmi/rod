use crate::message::Message;
use crate::actor::{Actor, ActorContext};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;

use async_trait::async_trait;

use futures_util::{future, TryStreamExt};
use log::{info, error, debug};

use tokio_tungstenite::{WebSocketStream, MaybeTlsStream};
use tokio_tungstenite::tungstenite::{Message as WsMessage};

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type WsSender = SplitSink<WsStream, WsMessage>;
type WsReceiver = SplitStream<WsStream>;

pub struct WsConn {
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
    async fn handle(&mut self, msg: Message, _ctx: &ActorContext) {
        let _ = self.sender.send(WsMessage::Text(msg.to_string())).await;
    }

    async fn pre_start(&mut self, ctx: &ActorContext) {
        info!("WsConn starting");
        let receiver = self.receiver.take().unwrap();
        let ctx2 = ctx.clone();
        ctx.abort_on_stop(tokio::spawn(async move {
            let _ = receiver.try_for_each(|msg| {
                if let Ok(s) = msg.to_text() {
                    match Message::try_from(s, ctx2.addr.clone()) {
                        Ok(msgs) => {
                            debug!("ws_conn in");
                            for msg in msgs.into_iter() {
                                if let Err(e) = ctx2.router.sender.send(msg) {
                                    error!("failed to send incoming message to node: {}", e);
                                }
                            }
                        },
                        _ => {}
                    };
                }
                future::ok(())
            }).await;
        }));
    }

    async fn stopping(&mut self, _context: &ActorContext) {
        info!("WsConn stopping");
    }
}
