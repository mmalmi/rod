use multicast_socket::MulticastSocket;
use std::net::{SocketAddrV4};

use crate::types::NetworkAdapter;
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Multicast {
    node: Node,
    socket: Arc<RwLock<MulticastSocket>>
}

#[async_trait]
impl NetworkAdapter for Multicast {
    fn new(node: Node) -> Self {
        let socket_addr = SocketAddrV4::new([233, 255, 255, 255].into(), 7654);
        let socket = MulticastSocket::all_interfaces(socket_addr)
            .expect("could not create and bind multicast socket");
        let socket = Arc::new(RwLock::new(socket));
        Multicast { node, socket }
    }

    async fn start(&self) { // "wss://gun-us.herokuapp.com/gun"
        debug!("Syncing over multicast\n");

        let mut node = self.node.clone();
        let mut rx = node.get_outgoing_msg_receiver();
        let socket = self.socket.clone();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = socket.read().await.receive() {
                    // TODO if message.from == multicast_[interface], don't resend to [interface]
                    if let Ok(data) = std::str::from_utf8(&message.data) {
                        let uid = format!("multicast_{:?}", message.interface).to_string();
                        node.incoming_message(data.to_string(), &uid);
                    }
                };
            }
        });

        let socket = self.socket.clone();
        tokio::task::spawn(async move {
            while let Ok(message) = rx.recv().await { // TODO loop and handle rx closed
                if let Err(e) = socket.write().await.broadcast(message.msg.as_bytes()) {
                    error!("multicast send error {}", e);
                }
            }
        });
    }

    fn stop(&self) {

    }
}


