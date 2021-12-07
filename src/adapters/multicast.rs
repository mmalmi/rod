use multicast_socket::MulticastSocket;
use std::net::{SocketAddrV4};

use crate::types::NetworkAdapter;
use crate::Node;
use async_trait::async_trait;
use log::{debug, error};
use std::sync::{Arc, RwLock};

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
        let socket = self.socket.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                if let Ok(message) = socket.read().unwrap().receive() {
                    if let Ok(data) = std::str::from_utf8(&message.data) {
                        let uid = format!("multicast_{:?}", message.interface).to_string();
                        node.incoming_message(data.to_string(), &uid);
                    }
                };
            }
        });
    }

    fn stop(&self) {

    }

    fn send_str(&self, m: &String, _from: &String) -> () {
        let m = m.clone();
        let socket = self.socket.clone();
        tokio::task::spawn(async move { // TODO instead, send a message to a sender task via bounded channel
            match socket.write().unwrap().broadcast(m.as_bytes()) {
                Ok(_) => {},
                Err(e) => error!("multicast send error {}", e)
            }
        });
    }
}


