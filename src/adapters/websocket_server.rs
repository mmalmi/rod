use actix::{Actor, Running, StreamHandler, AsyncContext, Handler, Addr}; use actix_web::web::Data;
// would much rather use warp, but its dependency "hyper" has a memory leak https://github.com/hyperium/hyper/issues/1790
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, middleware};
use actix_web_actors::ws;
use actix_files as fs;

use actix_web::error::PayloadError;
use ws::{handshake, WebsocketContext};
use actix_http::ws::{Codec, Message, ProtocolError};
use bytes::Bytes;
use futures::Stream;

use std::collections::HashMap;
use async_trait::async_trait;
use crate::message::Message as MyMessage;
use crate::actor::{Actor as MyActor, Addr as MyAddr, ActorContext};
use crate::Config;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Instant, Duration};
use tokio::sync::RwLock;
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};

//use tokio::time::sleep;

use log::{debug, error, info};

static NEXT_USER_ID: AtomicUsize = AtomicUsize::new(1);
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

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
pub struct MyWs {
    id: String,
    users: Users,
    ctx: ActorContext,
    actix_addr: Option<Addr<MyWs>>,
    heartbeat: Instant
}

impl MyWs {
    fn check_and_send_heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |actor, ctx| {
            if Instant::now().duration_since(actor.heartbeat) > CLIENT_TIMEOUT {
                ctx.close(None);
                return;
            }
            ctx.ping(b"PING");
        });
    }
}

pub struct OutgoingMessage {
    pub str: String
}

impl actix::Message for OutgoingMessage {
    type Result = ();
}

pub struct MyWsHelper {
    pub addr: Addr<MyWs>
}

#[async_trait]
impl MyActor for MyWsHelper {
    async fn pre_start(&mut self, _ctx: &ActorContext) {}
    async fn handle(&mut self, msg: MyMessage, ctx: &ActorContext) {
        debug!("forwarding to Actix Addr");
        if let Err(_) = self.addr.try_send(OutgoingMessage { str: msg.to_string() }) {
            ctx.stop();
        }
    }
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.set_mailbox_capacity(1000);
        ctx.text(format!("[{{\"dam\":\"hi\",\"#\":\"{}\"}}]", self.ctx.peer_id));
        self.actix_addr = Some(ctx.address());
        self.check_and_send_heartbeat(ctx);
        let id = self.id.clone();
        let users = self.users.clone();
        let addr = ctx.address();
        let my_addr = self.ctx.addr.clone();

        let helper = MyWsHelper { addr: addr.clone() };
        self.ctx.start_actor(Box::new(helper));

        tokio::task::spawn(async move {
            let mut users = users.write().await;
            users.insert(id.clone(), (addr, my_addr));
        });
    }

    fn stopping(&mut self, _ctx: &mut ws::WebsocketContext<Self>) -> Running {
        let users = self.users.clone();
        let id = self.id.clone();
        tokio::task::spawn(async move {
            let mut users = users.write().await;
            users.remove(&id);
        });
        Running::Stop
    }
}

impl Handler<OutgoingMessage> for MyWs {
    type Result = ();

    fn handle(&mut self, msg: OutgoingMessage, ctx: &mut Self::Context) {
        debug!("out {}", msg.str);
        ctx.text(msg.str);
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
            Ok(ws::Message::Ping(msg)) => ctx.pong(&msg), // TODO: send pings, and close connection if they don't pong on time?
            Ok(ws::Message::Pong(_)) => self.heartbeat = Instant::now(),
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
            },
            Ok(ws::Message::Text(text)) => {
                debug!("in from client: {}", text);
                match MyMessage::try_from(&text.to_string(), self.ctx.addr.clone()) {
                    Ok(msgs) => {
                        for msg in msgs.into_iter() {
                            if let Err(e) = self.ctx.router.sender.send(msg) {
                                error!("error sending incoming message to router: {}", e);
                            }
                        }
                    },
                    _ => {}
                    //Err(e) => debug!("{}", e)
                };
            },
            Err(e) => {
                error!("error receiving from websocket: {}", e);
                ctx.close(None);
            },
            _ => debug!("received non-text msg"),
        }
    }
}

struct AppState {
    peer_id: String,
}

type Users = Arc<RwLock<HashMap<String, (Addr<MyWs>, MyAddr)>>>;

pub struct WebsocketServer {
    users: Users,
    config: Config
}

#[async_trait]
impl MyActor for WebsocketServer {
    async fn pre_start(&mut self, ctx: &ActorContext) {
        info!("WebsocketServer adapter starting");
        let users = self.users.clone();

        /*
        let users_clone = users.clone();
        let update_stats = self.config.stats;
        if update_stats {
            tokio::task::spawn(async move {
                loop { // TODO break
                    let users = users_clone.read().await;
                    node_clone.get("node_stats").get(&peer_id).get("websocket_server_connections").put(users.len().to_string().into());
                    sleep(tokio::time::Duration::from_millis(1000)).await;
                }
            });
        }
         */

        let users = self.users.clone();
        let config = self.config.clone();
        let ctx = ctx.clone();
        tokio::spawn(async move {
            Self::actix_start(config, users, ctx).await.unwrap(); // TODO close when receiver dropped
        });
    }

    async fn handle(&mut self, message: MyMessage, ctx: &ActorContext) {
        match message {
            MyMessage::Get(ref msg) => { self.broadcast(message).await; },
            MyMessage::Put(ref msg) => { self.broadcast(message).await; },
            _ => {}
        }
    }
}

impl WebsocketServer {
    pub fn new(config: Config) -> Self {
        WebsocketServer {
            users: Users::default(),
            config
        }
    }

    fn actix_start(config: Config, users: Users, ctx: ActorContext) -> actix_web::dev::Server {
        let url = format!("0.0.0.0:{}", config.websocket_server_port);

        let users_clone = users.clone();
        let config_clone = config.clone();
        let server = HttpServer::new(move || {
            let users = users_clone.clone();
            let config_clone = config_clone.clone();
            let ctx = ctx.clone();
            App::new()
                .app_data(Data::new(AppState { peer_id: ctx.peer_id.clone() }))
                .wrap(middleware::Logger::default())
                .route("/peer_id", web::get().to(Self::peer_id))
                .service(fs::Files::new("/stats", "assets/stats").index_file("index.html"))
                .route("/gun", web::get().to(
                    move |a, b| {
                        Self::user_connected(a, b, config_clone.websocket_frame_max_size, users.clone(), ctx.clone())
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

    async fn broadcast(&self, message: MyMessage) {
        let msg_str = message.clone().to_string();
        for (actix_addr, my_addr) in self.users.read().await.values() {
            if message.is_from(my_addr) {
                continue;
            }
            if let Err(e) = actix_addr.try_send(OutgoingMessage { str: msg_str.clone() }) {
                error!("error sending outgoing msg to websocket actor: {}", e);
            }
        }
    }

    async fn peer_id(data: web::Data<AppState>) -> String {
        data.peer_id.clone()
    }

    async fn user_connected(req: HttpRequest,
                            stream: web::Payload,
                            websocket_frame_max_size: usize,
                            users: Users,
                            ctx: ActorContext
    ) -> Result<HttpResponse, Error> {
        // Use a counter to assign a new unique ID for this user.
        let id = NEXT_USER_ID.fetch_add(1, Ordering::Relaxed);
        let id = format!("ws_server_{}", id).to_string();

        let ws = MyWs {
            id,
            users,
            ctx,
            actix_addr: None,
            heartbeat: Instant::now()
        };

        let resp = start_with_codec(ws, &req, stream, Codec::new().max_size(websocket_frame_max_size));

        //println!("{:?}", resp);
        resp
    }
}
