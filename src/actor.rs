use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use crate::message::Message;
use crate::utils::random_string;
use crate::Node;
use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::sync::oneshot;

// TODO: stop signal. Or just call tokio runtime stop / abort? https://docs.rs/tokio/1.18.2/tokio/task/struct.JoinHandle.html#method.abort

/// Our very own actor framework. Kudos to https://ryhl.io/blog/actors-with-tokio/
///
/// Actors should relay messages to [Node::get_router_addr]
#[async_trait]
pub trait Actor {
    /// This is called on node.start_adapters()
    async fn handle(&self, message: Message, context: &ActorContext);
}
impl Actor {
    async fn run(&self, receiver: Receiver<Message>, stop_receiver: oneshot::Receiver<()>, context: ActorContext) {
        self.started(&context).await;

        loop {
            tokio::select! {
                Some(()) = stop_receiver.recv() => {
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
        self.stopping(&context);
    }
    async fn started(&self, _context: &ActorContext) {}
    async fn stopping(&self, _context: &ActorContext) {}
}

/// Stuff that Actors need (cocaine not included)
pub struct ActorContext {
    pub addr: Weak<Addr>, // Weak reference so that addr.sender doesn't linger in the context of Actor::run()
    pub stop_signal: oneshot::Sender<()>,
    pub peer_id: String,
    pub router: Addr,
}
impl ActorContext {
    pub fn new(&self, addr: Weak<Addr>, stop_signal: oneshot::Sender<()>) -> Self {
        Self {
            addr,
            stop_signal,
            peer_id: self.peer_id.clone(),
            router: self.router.clone()
        }
    }
}

pub fn start_actor(actor: Box<Actor>, mut context: ActorContext) -> Arc<Addr> {
    let (sender, receiver) = channel::new(10);
    let (stop_sender, stop_receiver) = oneshot::channel();
    let addr = Arc::new(Addr::new(sender));
    let new_context = context.new(Arc::downgrade(addr), stop_sender);
    tokio::spawn(async move { actor.run(receiver, stop_receiver, new_context).await }); // ActorSystem with HashMap<Addr, Sender> that lets us call stop() on all actors?
    addr
}

#[derive(Clone, Debug)]
pub struct Addr {
    id: String,
    pub sender: Sender<Message>
}
impl Addr {
    pub fn new(sender: Sender<Message>) -> Self {
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

