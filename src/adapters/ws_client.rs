use futures_util::{StreamExt};
use tokio_tungstenite::{
    connect_async,
};
use url::Url;
use std::collections::HashMap;

use crate::adapters::ws_conn::WsConn;
use crate::message::Message;
use crate::actor::{Actor, Addr, ActorContext};
use crate::Config;
use async_trait::async_trait;
use log::{debug, info};
use tokio::time::{sleep, Duration};

pub struct OutgoingWebsocketManager {
    clients: HashMap<String, Addr>,
    config: Config
}

impl OutgoingWebsocketManager {
    pub fn new(config: Config) -> Self {
        OutgoingWebsocketManager {
            config,
            clients: HashMap::new()
        }
    }
}

#[async_trait]
impl Actor for OutgoingWebsocketManager { // TODO: support multiple outbound websockets
    async fn pre_start(&mut self, ctx: &ActorContext) {
        info!("OutgoingWebsocketManager starting");
        for url in self.config.outgoing_websocket_peers.iter() {
            let url = url.to_string();
            loop { // TODO break on actor shutdown
                sleep(Duration::from_millis(1000)).await;
                if self.clients.contains_key(&url) {
                    continue;
                }
                let result = connect_async(
                    Url::parse(&url).expect("Can't connect to URL"),
                ).await;
                if let Ok(tuple) = result {
                    let (socket, _) = tuple;
                    debug!("outgoing websocket opened to {}", url);
                    let (sender, receiver) = socket.split();
                    let client = WsConn::new(sender, receiver, self.config.allow_public_space);
                    let addr = ctx.start_actor(Box::new(client));
                    self.clients.insert(url.clone(), addr);
                }
            }
        }
    }

    async fn handle(&mut self, message: Message, _ctx: &ActorContext) {
        self.clients.retain(|_url,client| {
            client.send(message.clone()).is_ok()
        });
    }
}

