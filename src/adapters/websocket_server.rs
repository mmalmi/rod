use actix::{Actor, StreamHandler, AsyncContext, Handler}; // would much rather use warp, but its dependency "hyper" has a memory leak https://github.com/hyperium/hyper/issues/1790
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, middleware};
use actix_web_actors::ws;
use actix_files as fs;

use actix_web::error::PayloadError;
use ws::{handshake, WebsocketContext};
use actix_http::ws::{Codec, Message, ProtocolError};
use bytes::Bytes;
use futures::Stream;

use std::collections::HashSet;
use async_trait::async_trait;
use crate::types::{NetworkAdapter, GunMessage};
use crate::Node;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

use log::{debug, error};

static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

// this is needed to set a higher websocket frame size in a custom Codec
fn start_with_codec<A, S>(actor: A, req: &HttpRequest, stream: S, codec: Codec) -> Result<HttpResponse, Error>
where
    A: Actor<Context = WebsocketContext<A>>
        + StreamHandler<Result<Message, ProtocolError>>,
    S: Stream<Item = Result<Bytes, PayloadError>> + 'static,
{
    let mut res = handshake(req)?;
    Ok(res.streaming(WebsocketContext::with_codec(actor, stream, codec)))
}

/// Define HTTP actor
struct MyWs {
    node: Node,
    id: String,
    users: Users,
    incoming_msg_sender: tokio::sync::mpsc::Sender<GunMessage>
}

struct OutgoingMessage {
    gun_message: GunMessage
}

impl actix::Message for OutgoingMessage {
    type Result = ();
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        // TODO say hi [{"dam":"hi","#":"iED196J6w"}]
        let mut rx = self.node.get_outgoing_msg_receiver();
        let id = self.id.clone();
        self.users.write().unwrap().insert(id.clone());
        let addr = ctx.address();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await { // TODO: single thread for all actors, then IncomingMessages per recipient
                    if message.from == id {
                        continue;
                    }
                    if let Some(to) = message.to.clone() {
                        if !to.contains(&id) {
                            continue;
                        }
                    }
                    let res = addr.send(OutgoingMessage { gun_message: message }).await; // TODO break on Closed error only
                    if let Err(e) = res {
                        if let actix::prelude::MailboxError::Closed = e {
                            break;
                        }
                    }
                }
            }
        });
        self.update_stats();
    }

    fn stopped(&mut self, _ctx: &mut ws::WebsocketContext<Self>) {
        self.users.write().unwrap().remove(&self.id);
        self.update_stats();
    }
}

impl MyWs {
    fn update_stats(&mut self) {
        let peer_id = self.node.get_peer_id();
        self.node.get("node_stats").get(&peer_id).get("websocket_server_connections").put(self.users.read().unwrap().len().to_string().into());
    }
}

impl Handler<OutgoingMessage> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: OutgoingMessage, ctx: &mut Self::Context) {
        let text = format!("{}", msg.gun_message.msg);
        debug!("out {}", text);
        ctx.text(text);
    }
}

/// Handler for ws::Message message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWs {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context,
    ) {
        match msg {
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg),
            Ok(ws::Message::Text(text)) => {
                debug!("in: {}", text);
                if let Err(e) = self.incoming_msg_sender.try_send(GunMessage { msg: text.to_string(), from: self.id.clone(), to: None }) {
                    error!("error sending incoming message to node: {}", e);
                }
            },
            _ => debug!("received non-text msg"),
        }
    }
}

struct AppState {
    peer_id: String,
}

type Users = Arc<RwLock<HashSet<String>>>;

pub struct WebsocketServer {
    node: Node,
    users: Users
}

#[async_trait]
impl NetworkAdapter for WebsocketServer {
    fn new(node: Node) -> Self {
        WebsocketServer {
            node,
            users: Users::default()
        }
    }

    async fn start(&self) {
        let node = self.node.clone();
        let users = self.users.clone();
        Self::actix_start(node, users).await.unwrap();
    }
}

impl WebsocketServer {
    fn actix_start(node: Node, users: Users) -> actix_web::dev::Server {
        let config = node.config.read().unwrap();
        let url = format!("0.0.0.0:{}", config.websocket_server_port);

        let node_clone = node.clone();
        let server = HttpServer::new(move || {
            let node = node_clone.clone();
            let users = users.clone();
            let peer_id = node.get_peer_id();
            App::new()
                .data(AppState { peer_id })
                .wrap(middleware::Logger::default())
                .route("/peer_id", web::get().to(Self::peer_id))
                .service(fs::Files::new("/stats", "assets/stats").index_file("index.html"))
                .route("/gun", web::get().to(
                    move |a, b| {
                        Self::user_connected(a, b, node.clone(), users.clone())
                    }
                ))
                .service(fs::Files::new("/", "assets/iris").index_file("index.html"))
        });

        if let Some(cert_path) = &config.cert_path {
            if let Some(key_path) = &config.key_path {
                let mut builder = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
                    builder
                        .set_private_key_file(key_path, SslFiletype::PEM)
                        .unwrap();
                    builder.set_certificate_chain_file(cert_path).unwrap();
                return server.bind_openssl(url, builder).unwrap().run()
            }
        }

        server.bind(url).unwrap().run()
    }

    async fn peer_id(data: web::Data<AppState>) -> String {
        data.peer_id.clone()
    }

    async fn user_connected(req: HttpRequest, stream: web::Payload, node: Node, users: Users) -> Result<HttpResponse, Error> {
        // Use a counter to assign a new unique ID for this user.
        let id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);
        let id = format!("ws_server_{}", id).to_string();

        let ws = MyWs { node: node.clone(), id, users, incoming_msg_sender: node.get_incoming_msg_sender() };

        let config = node.config.read().unwrap();
        let resp = start_with_codec(ws, &req, stream, Codec::new().max_size(config.websocket_frame_max_size));

        //println!("{:?}", resp);
        resp
    }
}
