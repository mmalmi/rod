use multicast_socket::MulticastSocket;
use std::net::{SocketAddrV4};

use crate::message::Message;
use crate::actor::{Actor, ActorContext};
use async_trait::async_trait;
use log::{info, debug, error};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Multicast {
    socket: Arc<RwLock<MulticastSocket>>
}

impl Multicast {
    pub fn new() -> Self {
        let socket_addr = SocketAddrV4::new([233, 255, 255, 255].into(), 7654);
        let socket = MulticastSocket::all_interfaces(socket_addr)
            .expect("could not create and bind multicast socket");
        let socket = Arc::new(RwLock::new(socket));
        Multicast { socket }
    }
}

#[async_trait]
impl Actor for Multicast {
    async fn handle(&mut self, msg: Message, ctx: &ActorContext) {
        match msg {
            Message::Put(put) => {
                if let Err(e) = self.socket.write().await.broadcast(put.to_string().as_bytes()) {
                    error!("multicast send error {}", e);
                }
            },
            Message::Get(get) => {
                if let Err(e) = self.socket.write().await.broadcast(get.to_string().as_bytes()) {
                    error!("multicast send error {}", e);
                }
            },
            _ => {}
        }
    }

    async fn pre_start(&mut self, ctx: &ActorContext) { // "wss://gun-us.herokuapp.com/gun"
        info!("Syncing over multicast\n");

        let socket = self.socket.clone();
        let addr = ctx.addr.clone();
        let router = ctx.router.clone();
        tokio::task::spawn(async move {
            loop { // TODO break on self.receiver close
                let addr = addr.clone();
                if let Ok(message) = socket.read().await.receive() {
                    // TODO if message.from == multicast_[interface], don't resend to [interface]
                    if let Ok(data) = std::str::from_utf8(&message.data) {
                        debug!("in: {}", data);
                        //let from = format!("multicast_{:?}", message.interface).to_string();
                        match Message::try_from(data, addr) {
                            Ok(msgs) => {
                                for msg in msgs.into_iter() {
                                    match msg {
                                        Message::Put(put) => {
                                            let put = put.clone();
                                            if let Err(e) = router.sender.send(Message::Put(put)) {
                                                error!("failed to send message to node: {}", e);
                                            }
                                        },
                                        Message::Get(get) => {
                                            let get = get.clone();
                                            if let Err(e) = router.sender.send(Message::Get(get)) {
                                                error!("failed to send message to node: {}", e);
                                            }
                                        },
                                        _ => {}
                                    }
                                }
                            },
                            Err(e) => error!("message parsing failed: {}", e)
                        }
                    }
                };
            }
        });
    }
}


