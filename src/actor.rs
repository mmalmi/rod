use crate::message::Message;
use crate::utils::random_string;
use crate::Node;
use async_trait::async_trait;
use futures_util::Future;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::Send;
use std::sync::{Arc, RwLock};
use tokio::sync::mpsc::{
    channel, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender,
};
use tokio::task::JoinHandle;

// TODO: stop signal. Or just call tokio runtime stop / abort? https://docs.rs/tokio/1.18.2/tokio/task/struct.JoinHandle.html#method.abort
// TODO make this a trait. Move platform / runtime specific stuff here, so different versions can be used on wasm.

/// Our very own actor framework. Kudos to https://ryhl.io/blog/actors-with-tokio/
///
/// Actors should relay messages to [Node::get_router_addr]
#[async_trait]
pub trait Actor: Send + Sync + 'static {
    async fn handle(&mut self, message: Message, context: &ActorContext);
    async fn pre_start(&mut self, _context: &ActorContext) {}
    async fn stopping(&mut self, _context: &ActorContext) {}
    /// Tells the router if this Actor wants to receive all messages (like the Multicast adapter)
    fn subscribe_to_everything(&self) -> bool {
        false
    }
}
impl dyn Actor {
    async fn run(
        &mut self,
        mut receiver: UnboundedReceiver<Message>,
        mut stop_receiver: Receiver<()>,
        mut context: ActorContext,
    ) {
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
    stop_signals: Arc<RwLock<HashMap<Addr, Sender<()>>>>,
    task_handles: Arc<RwLock<Vec<JoinHandle<()>>>>,
    pub addr: Addr,
    pub is_stopped: Arc<RwLock<bool>>,
    pub node: Option<Node>,
}
impl ActorContext {
    pub fn new(peer_id: String) -> Self {
        Self {
            addr: Addr::noop(),
            stop_signals: Arc::new(RwLock::new(HashMap::new())),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            peer_id: Arc::new(RwLock::new(peer_id)),
            router: Addr::noop(),
            is_stopped: Arc::new(RwLock::new(false)),
            node: None,
        }
    }

    pub fn child_actor_count(&self) -> usize {
        self.stop_signals.read().unwrap().len()
    }

    fn child_context(&self, addr: Addr, stop_signal: Sender<()>) -> Self {
        let mut stop_signals = HashMap::new();
        stop_signals.insert(addr.clone(), stop_signal);
        Self {
            addr,
            stop_signals: Arc::new(RwLock::new(stop_signals)),
            task_handles: Arc::new(RwLock::new(Vec::new())),
            peer_id: self.peer_id.clone(),
            router: self.router.clone(),
            is_stopped: self.is_stopped.clone(),
            node: self.node.clone(),
        }
    }

    pub fn start_actor(&self, actor: Box<dyn Actor>) -> Addr {
        self.start_actor_or_router(actor, false)
    }

    pub fn start_router(&self, actor: Box<dyn Actor>) -> Addr {
        self.start_actor_or_router(actor, true)
    }

    pub fn child_task<T>(&self, task: T)
    where
        T: Future<Output = ()> + Send + 'static,
    {
        let handle = tokio::spawn(task);
        self.task_handles.write().unwrap().push(handle);
    }

    pub fn blocking_child_task<F>(&self, task: F)
    where
        F: FnOnce() -> () + Send + 'static,
    {
        let handle = tokio::task::spawn_blocking(task);
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
        self.stop_signals
            .write()
            .unwrap()
            .insert(addr.clone(), stop_sender);
        let stop_signals = self.stop_signals.clone();
        let addr_clone = addr.clone();
        tokio::spawn(async move {
            actor.run(receiver, stop_receiver, new_context).await;
            stop_signals.write().unwrap().remove(&addr_clone);
        });
        addr
    }

    pub fn stop(&mut self) {
        for handle in self.task_handles.read().unwrap().iter() {
            handle.abort();
        }
        for signal in self.stop_signals.read().unwrap().values() {
            let _ = signal.try_send(());
        }
        self.node = None;
        *self.is_stopped.write().unwrap() = true;
    }
}

#[derive(Clone, Debug)]
pub struct Addr {
    id: String,
    sender: UnboundedSender<Message>,
}
impl Addr {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        Self {
            id: random_string(32),
            sender,
        }
    }

    pub fn send(&self, msg: Message) -> Result<(), ()> {
        match self.sender.send(msg) {
            Ok(_) => Ok(()),
            Err(_) => Err(()),
        }
    }

    /// Returns a no-op address
    pub fn noop() -> Addr {
        let (sender, _receiver) = unbounded_channel::<Message>();
        Addr::new(sender)
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
