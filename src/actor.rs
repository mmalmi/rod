use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use crate::message::Message;
use crate::utils::random_string;
use crate::Node;
use tokio::sync::mpsc::{Sender, Receiver};

/// Syncs the gun Node with other Nodes over various transports like websocket or multicast.
///
/// Actors should relay messages to [Node::get_router_addr]
#[async_trait]
pub trait Actor {
    fn new(receiver: Receiver<Message>, node: Node) -> Self where Self: Sized;
    /// This is called on node.start_adapters()
    async fn start(&self);
}

#[derive(Clone, Debug)]
pub struct Addr {
    id: String,
    pub sender: Sender<Message>
}
impl Addr {
    fn new(sender: Sender<Message>) -> Self {
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

