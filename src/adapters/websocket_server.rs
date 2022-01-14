use actix::{Actor, StreamHandler, AsyncContext, Handler}; // would rather use warp, but its dependency "hyper" has a memory leak
use actix_web::{web, App, Error, Responder, HttpRequest, HttpResponse, HttpServer, middleware};
use actix_web_actors::ws;
use actix_files as fs;
use std::collections::HashSet;
use std::env;
use async_trait::async_trait;
use crate::types::{NetworkAdapter, GunMessage};
use crate::Node;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);

/// Define HTTP actor
pub struct MyWs {
    node: Node,
    id: String,
    users: Users
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
        let mut rx = self.node.get_outgoing_msg_receiver();
        let id = self.id.clone();
        self.users.write().unwrap().insert(id.clone());
        let addr = ctx.address();
        tokio::task::spawn(async move {
            loop {
                if let Ok(message) = rx.recv().await {
                    if message.from == id {
                        continue;
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
            Ok(ws::Message::Text(text)) => self.node.incoming_message(text.to_string(), &self.id),
            _ => (),
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
        Self::actix_start(self.node.clone(), self.users.clone()).await;
    }

    fn stop(&self) {

    }
}

impl WebsocketServer {
    fn actix_start(node: Node, users: Users) -> actix_web::dev::Server {
        let port: u16 = match env::var("PORT") {
            Ok(p) => p.parse::<u16>().unwrap(),
            _ => 4944
        };
        let url = format!("0.0.0.0:{}", port);

        let server = HttpServer::new(move || {
            let node = node.clone();
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

        if let Ok(cert_path) = env::var("CERT_PATH") {
            if let Ok(key_path) = env::var("KEY_PATH") {
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

        let ws = MyWs { node, id, users };
        let resp = ws::start(ws, &req, stream);

        //println!("{:?}", resp);
        resp
    }
}
