use multicast_socket::{MulticastSocket, MulticastOptions, all_ipv4_interfaces};
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
        let bind_address = SocketAddrV4::new([233, 255, 255, 255].into(), 7654);
        let options = MulticastOptions {
            buffer_size: 64 * 1024,
            ..MulticastOptions::default()
        };
        let interfaces = all_ipv4_interfaces().expect("could not list multicast interfaces");
        let socket = MulticastSocket::with_options(bind_address, interfaces, options)
            .expect("could not create and bind multicast socket");
        let socket = Arc::new(RwLock::new(socket));
        Multicast { socket }
    }

    fn handle_incoming_message(data: &str, ctx: &ActorContext) {
        debug!("in: {}", data);
        //let from = format!("multicast_{:?}", message.interface).to_string();
        match Message::try_from(data, ctx.addr.clone()) {
            Ok(msgs) => {
                for msg in msgs.into_iter() {
                    match msg {
                        Message::Put(put) => {
                            let put = put.clone();
                            if let Err(e) = ctx.router.sender.send(Message::Put(put)) {
                                error!("failed to send message to node: {}", e);
                            }
                        },
                        Message::Get(get) => {
                            let get = get.clone();
                            if let Err(e) = ctx.router.sender.send(Message::Get(get)) {
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
}

#[async_trait]
impl Actor for Multicast {
    async fn handle(&mut self, msg: Message, _ctx: &ActorContext) {
        debug!("multicast out msg {}", msg.get_id());
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
        let ctx_clone = ctx.clone();
        ctx.abort_on_stop(tokio::spawn(async move {
            while let Ok(message) = socket.read().await.receive() {
                // TODO if message.from == multicast_[interface], don't resend to [interface]
                if let Ok(data) = std::str::from_utf8(&message.data) {
                    Self::handle_incoming_message(data, &ctx_clone);
                }
            }
        }));
    }
}


