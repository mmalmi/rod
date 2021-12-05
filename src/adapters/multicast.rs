use multicast_socket::MulticastSocket;
use std::net::{SocketAddrV4, Ipv4Addr};

use futures_util::{SinkExt, StreamExt};
use std::env;

use crate::types::NetworkAdapter;
use crate::Node;
use async_trait::async_trait;
use log::{debug};
use std::sync::{Arc, RwLock};
use std::{thread, time};

pub struct Multicast {
    node: Node,
    socket: Arc<RwLock<MulticastSocket>>
}

#[async_trait]
impl NetworkAdapter for Multicast {
    fn new(node: Node) -> Self {
        let socket_addr = SocketAddrV4::new([224, 0, 0, 251].into(), 7654);
        let socket = MulticastSocket::all_interfaces(socket_addr)
            .expect("could not create and bind multicast socket");
        let socket = Arc::new(RwLock::new(socket));
        Multicast { node, socket }
    }

    async fn start(&self) { // "wss://gun-us.herokuapp.com/gun"
        debug!("Syncing over multicast\n");

        let mut node = self.node.clone();
        let socket = self.socket.clone();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = socket.read().unwrap().receive() {
                    dbg!(&message.interface);
                    dbg!(&message.origin_address);
                    if let Ok(data) = std::str::from_utf8(&message.data) {
                        node.incoming_message(data.to_string(), &"multicast-interface-x".to_string());
                    }
                };
            }
        });
    }

    fn stop(&self) {

    }

    fn send_str(&self, m: &String, from: &String) -> () {
        debug!("send");
        let m = m.clone();
        let socket = self.socket.clone();
        tokio::task::spawn(async move { // TODO instead, send a message to a sender task via bounded channel
            socket.write().unwrap().broadcast(m.as_bytes());
        });
    }
}


