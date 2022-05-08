use multicast_socket::MulticastSocket;
use std::net::{SocketAddrV4};

use crate::message::Message;
use crate::actor::Actor;
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Receiver;

pub struct Multicast {
    node: Node,
    receiver: Receiver<Message>,
    socket: Arc<RwLock<MulticastSocket>>
}

#[async_trait]
impl Actor for Multicast {
    fn new(receiver: Receiver<Message>, node: Node) -> Self {
        let socket_addr = SocketAddrV4::new([233, 255, 255, 255].into(), 7654);
        let socket = MulticastSocket::all_interfaces(socket_addr)
            .expect("could not create and bind multicast socket");
        let socket = Arc::new(RwLock::new(socket));
        Multicast { node, receiver, socket }
    }

    async fn start(&self) { // "wss://gun-us.herokuapp.com/gun"
        debug!("Syncing over multicast\n");

        let socket = self.socket.clone();
        let router = self.node.get_router_addr().unwrap();
        tokio::task::spawn(async move {
            loop { // TODO break on self.receiver close
                if let Ok(message) = socket.read().await.receive() {
                    // TODO if message.from == multicast_[interface], don't resend to [interface]
                    if let Ok(data) = std::str::from_utf8(&message.data) {
                        debug!("in: {}", data);
                        let from = format!("multicast_{:?}", message.interface).to_string();
                        match Message::try_from(data, from) {
                            Ok(msgs) => {
                                for msg in msgs.into_iter() {
                                    match msg {
                                        Message::Put(put) => {
                                            let put = put.clone();
                                            if let Err(e) = router.sender.try_send(Message::Put(put)) {
                                                error!("failed to send message to node: {}", e);
                                            }
                                        },
                                        Message::Get(get) => {
                                            let get = get.clone();
                                            if let Err(e) = router.sender.try_send(Message::Get(get)) {
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

        let socket = self.socket.clone();
        tokio::task::spawn(async move {
            while let Some(msg) = self.receiver.recv().await { // TODO loop and handle rx closed
                //debug!("out {}", msg);

                match msg {
                    Message::Put(put) => {
                        if let Err(e) = socket.write().await.broadcast(put.to_string().as_bytes()) {
                            error!("multicast send error {}", e);
                        }
                    },
                    Message::Get(get) => {
                        if let Err(e) = socket.write().await.broadcast(get.to_string().as_bytes()) {
                            error!("multicast send error {}", e);
                        }
                    },
                    _ => {}
                }
            }
        });
    }
}


