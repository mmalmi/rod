use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Weak};
use std::fmt;
use std::marker::Send;
use crate::message::Message;
use crate::utils::random_string;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::oneshot;
use log::{info};

// TODO: stop signal. Or just call tokio runtime stop / abort? https://docs.rs/tokio/1.18.2/tokio/task/struct.JoinHandle.html#method.abort

/// Our very own actor framework. Kudos to https://ryhl.io/blog/actors-with-tokio/
///
/// Actors should relay messages to [Node::get_router_addr]
#[async_trait]
pub trait Actor: Send + Sync + 'static {
    /// This is called on node.start_adapters()
    async fn handle(&mut self, message: Message, context: &ActorContext);
    async fn pre_start(&mut self, _context: &ActorContext) {}
    async fn stopping(&mut self, _context: &ActorContext) {}

}
impl dyn Actor {
    async fn run(&mut self, mut receiver: UnboundedReceiver<Message>, mut stop_receiver: oneshot::Receiver<()>, context: ActorContext) {
        self.pre_start(&context).await;
        loop {
            tokio::select! {
                _v = &mut stop_receiver => {
                    break;
                },
                opt_msg = receiver.recv() => {
                    let msg = match opt_msg {
                        Some(msg) => msg,
                        None => break,
                    };
                    self.handle(msg, &context).await
                }
            }
        }
        self.stopping(&context).await;
    }
}

/// Stuff that Actors need (cocaine not included)
pub struct ActorContext {
    pub addr: Weak<Addr>, // Weak reference so that addr.sender doesn't linger in the context of Actor::run(). TODO: use just Addr and stop signals?
    pub stop_signal: oneshot::Sender<()>,
    pub peer_id: String,
    pub router: Addr,
}
impl ActorContext {
    pub fn new_with(&self, addr: Weak<Addr>, stop_signal: oneshot::Sender<()>) -> Self {
        Self {
            addr,
            stop_signal,
            peer_id: self.peer_id.clone(),
            router: self.router.clone()
        }
    }
}

pub fn start_actor(mut actor: Box<dyn Actor>, parent_context: &ActorContext) -> Arc<Addr> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let (stop_sender, stop_receiver) = oneshot::channel();
    let addr = Arc::new(Addr::new(sender));
    let new_context = parent_context.new_with(Arc::downgrade(&addr), stop_sender);
    tokio::spawn(async move { actor.run(receiver, stop_receiver, new_context).await }); // ActorSystem with HashMap<Addr, Sender> that lets us call stop() on all actors?
    addr
}

pub fn start_router(mut actor: Box<dyn Actor>, peer_id: String) -> Arc<Addr> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let (stop_sender, stop_receiver) = oneshot::channel();
    let addr = Arc::new(Addr::new(sender));
    let ctx = ActorContext {
        addr: Arc::downgrade(&addr),
        router: (*addr).clone(),
        peer_id,
        stop_signal: stop_sender
    };
    tokio::spawn(async move { actor.run(receiver, stop_receiver, ctx).await }); // ActorSystem with HashMap<Addr, Sender> that lets us call stop() on all actors?
    addr
}

#[derive(Clone, Debug)]
pub struct Addr {
    id: String,
    pub sender: UnboundedSender<Message>
}
impl Addr {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        Self {
            id: random_string(32),
            sender
        }
    }
}
impl PartialEq for Addr {
    fn eq(&self, other: &Addr) -> bool {
        self.id == other.id
    }
}
impl Eq for Addr {}
impl Hash for Addr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "actor:{}", self.id)
    }
}
