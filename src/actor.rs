use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use std::fmt;
use std::marker::Send;
use std::sync::{Arc, RwLock};
use crate::message::Message;
use crate::utils::random_string;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver, Receiver, Sender, channel, unbounded_channel};
use tokio::task::JoinHandle;

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
    async fn run(&mut self, mut receiver: UnboundedReceiver<Message>, mut stop_receiver: Receiver<()>, context: ActorContext) {
        self.pre_start(&context).await;
        loop {
            tokio::select! {
                _v = stop_receiver.recv() => {
                    context.stop();
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
#[derive(Clone)]
pub struct ActorContext {
    pub peer_id: Arc<RwLock<String>>,
    pub router: Addr,
    stop_signals: Arc<RwLock<Vec<Sender<()>>>>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    pub addr: Addr,
    pub is_stopped: Arc<RwLock<bool>>
}
impl ActorContext {
    pub fn new(peer_id: String) -> Self {
        let (sender, _receiver) = unbounded_channel::<Message>();
        let noop = Addr::new(sender);
        Self {
            addr: noop.clone(),
            stop_signals: Arc::new(RwLock::new(Vec::new())),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            peer_id: Arc::new(RwLock::new(peer_id)),
            router: noop,
            is_stopped: Arc::new(RwLock::new(false))
        }
    }

    fn child_context(&self, addr: Addr, stop_signal: Sender<()>) -> Self {
        Self {
            addr,
            stop_signals: Arc::new(RwLock::new(vec![stop_signal])),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            peer_id: self.peer_id.clone(),
            router: self.router.clone(),
            is_stopped: self.is_stopped.clone()
        }
    }

    pub fn start_actor(&self, actor: Box<dyn Actor>) -> Addr {
        self.start_actor_or_router(actor, false)
    }

    pub fn start_router(&self, actor: Box<dyn Actor>) -> Addr {
        self.start_actor_or_router(actor, true)
    }

    pub fn abort_on_stop(&self, handle: JoinHandle<()>) {
        self.task_handles.write().unwrap().push(handle);
    }

    fn start_actor_or_router(&self, mut actor: Box<dyn Actor>, is_router: bool) -> Addr {
        let (sender, receiver) = unbounded_channel::<Message>();
        let (stop_sender, stop_receiver) = channel(1);
        let addr = Addr::new(sender);
        let mut new_context = self.child_context(addr.clone(), stop_sender.clone());
        if is_router {
            new_context.router = addr.clone();
        }
        self.stop_signals.write().unwrap().push(stop_sender);
        tokio::spawn(async move { actor.run(receiver, stop_receiver, new_context).await }); // ActorSystem with HashMap<Addr, Sender> that lets us call stop() on all actors?
        addr
    }

    pub fn stop(&self) {
        for handle in self.task_handles.read().unwrap().iter() {
            handle.abort();
        }
        for signal in self.stop_signals.read().unwrap().iter() {
            let _ = signal.try_send(());
        }
        *self.is_stopped.write().unwrap() = true;
    }
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
