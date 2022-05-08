use crate::message::{Message, Put, Get};
use crate::types::*;
use crate::actor::Actor;
use crate::{Node, Config};
use crate::adapters::SledStorage;
use crate::adapters::MemoryStorage;
use crate::adapters::WebsocketServer;
use crate::adapters::WebsocketClient;
use crate::adapters::Multicast;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use sysinfo::{ProcessorExt, System, SystemExt};
use async_trait::async_trait;
use tokio::time::{sleep, Duration};

static SEEN_MSGS_MAX_SIZE: usize = 10000;

struct SeenGetMessage {
    from: String,
    last_reply_checksum: Option<String>,
}

pub struct Router {
    config: Config,
    adapters: HashMap<String, Box<dyn Actor + Send + Sync>>,
    seen_messages: BoundedHashSet,
    seen_get_messages: BoundedHashMap<String, SeenGetMessage>,
    subscribers_by_topic: HashMap<String, HashSet<String>>,
    msg_counter: AtomicUsize,
    peer_id: String,
    receiver: Receiver<Message>,
    addr: Weak<Addr>,
    subscriptions_by_node_id: HashMap<String, Vec<broadcast::Sender<GunValue>>>
}

#[async_trait]
impl Actor for Router {
    fn new(receiver: mpsc::Receiver<Message>, addr: Weak<Addr>, node: Node) -> Self {
        let outgoing_channel = broadcast::channel::<Message>(config.rust_channel_size);
        let router = Self {
            config: node.config.read().unwrap().clone(),
            adapters: HashMap::new(),
            seen_messages: BoundedHashSet::new(SEEN_MSGS_MAX_SIZE),
            seen_get_messages: BoundedHashMap::new(SEEN_MSGS_MAX_SIZE),
            subscribers_by_topic: HashMap::new(),
            peer_id: random_string(16),
            msg_counter: AtomicUsize::new(0),
            addr,
            receiver,
            subscriptions_by_node_id: Arc::new(RwLock::new(HashMap::new())),
        };
        if config.multicast {
            let multicast = Multicast::new(node.clone());
            router.adapters.insert("multicast".to_string(), Box::new(multicast));
        }
        if config.websocket_server {
            let server = WebsocketServer::new(node.clone());
            router.adapters.insert("ws_server".to_string(), Box::new(server));
        }
        if config.sled_storage {
            let sled_storage = SledStorage::new(node.clone());
            router.adapters.insert("sled_storage".to_string(), Box::new(sled_storage));
        }
        if config.memory_storage {
            let memory_storage = MemoryStorage::new(node.clone());
            router.adapters.insert("memory_storage".to_string(), Box::new(memory_storage));
        }
        let client = WebsocketClient::new(node.clone());
        router.adapters.insert("ws_client".to_string(), Box::new(client));
        // should we start right away?
        router
    }

    /// Listen to incoming messages and start [Actor]s
    async fn start(&self) {
        let mut futures = Vec::new();

        futures.push(async move {
            while let Some(msg) = self.receiver.recv().await {
                debug!("incoming message");
                match msg {
                    Message::Put(put) => self.handle_put(put),
                    Message::Get(get) => self.handle_get(get),
                    _ => {}
                }
            }
        });

        for adapter in self.adapters.values() {
            futures.push(adapter.start()); // adapters must be non-blocking: use async functions or spawn_blocking
        }
        if self.config.stats {
            self.update_stats();
        }

        futures::future::join_all(futures).await;
    }
}

impl Router {
    fn update_stats(&self) {
        let node = self.node.clone();
        let peer_id = node.get_peer_id();
        let mut stats = node.get("node_stats").get(&peer_id);
        let start_time = Instant::now();
        tokio::task::spawn(async move {
            let mut sys = System::new_all();
            loop {
                sys.refresh_all();
                stats.get("msgs_per_second").put(node.msg_counter.load(Ordering::Relaxed).into());
                node.msg_counter.store(0, Ordering::Relaxed);
                stats.get("total_memory").put(format!("{} MB", sys.total_memory() / 1000).into());
                stats.get("used_memory").put(format!("{} MB", sys.used_memory() / 1000).into());
                stats.get("cpu_usage").put(format!("{} %", sys.global_processor_info().cpu_usage() as u64).into());
                let uptime_secs = start_time.elapsed().as_secs();
                let uptime;
                if uptime_secs <= 60 {
                    uptime = format!("{} seconds", uptime_secs);
                } else if uptime_secs <= 2 * 60 * 60 {
                    uptime = format!("{} minutes", uptime_secs / 60);
                } else {
                    uptime = format!("{} hours", uptime_secs / 60 / 60);
                }
                stats.get("process_uptime").put(uptime.into());
                sleep(Duration::from_millis(1000)).await;
            }
        });
    }

    // record subscription & relay
    fn handle_get(&mut self, msg: Get) {
        if !msg.id.chars().all(char::is_alphanumeric) {
            error!("id {}", msg.id);
        }
        if self.is_message_seen(msg.id) {
            return;
        }
        let seen_get_message = SeenGetMessage { from: msg.from.clone(), last_reply_checksum: None };
        self.seen_get_messages.insert(msg.id.clone(), seen_get_message);
        let topic = msg.node_id.split("/").next().unwrap_or("");
        debug!("{} subscribed to {}", msg.from, topic);
        self.subscribers_by_topic.entry(topic.to_string())
            .or_insert_with(HashSet::new).insert(msg.from.clone());
        let id = msg.id.clone();
        self.send(Message::Get(msg), id);
    }

    // relay to original requester or all subscribers
    fn handle_put(&mut self, msg: Put) {
        if self.is_message_seen(msg.id) {
            return;
        }
        let mut recipients = HashSet::<String>::new();

        match &msg.in_response_to {
            Some(in_response_to) => {
                if let Some(seen_get_message) = self.seen_get_messages.get_mut(in_response_to) {
                    if msg.checksum != None && msg.checksum == seen_get_message.last_reply_checksum {
                        debug!("same reply already sent");
                        return;
                    } // failing these conditions, should we still send the ack to someone?
                    seen_get_message.last_reply_checksum = msg.checksum.clone();
                    recipients.insert(seen_get_message.from.clone());
                }
            },
            _ => {
                for node_id in msg.updated_nodes.keys() {
                    let topic = node_id.split("/").next().unwrap_or("");
                    if let Some(subscribers) = self.subscribers_by_topic.get(topic) {
                        recipients.extend(subscribers.clone());
                    }
                    debug!("getting subscribers for topic {}: {:?}", topic, recipients);
                }
            }
        };
        let mut msg = msg.clone();
        msg.recipients = Some(recipients);
        let id = msg.id.clone();
        self.send(Message::Put(msg), id);
    }

    fn is_message_seen(&mut self, id: &String) -> bool {
        self.msg_counter.fetch_add(1, Ordering::Relaxed);

        if self.seen_messages.contains(id) {
            debug!("already seen message {}", id);
            return true;
        }
        self.seen_messages.insert(id.clone());

        return false;
    }

    fn send(&self, msg: Message) {
        debug!("msg out {:?}", msg);
        self.seen_messages.insert(msg.get_id()); // TODO: doesn't seem to work, at least on multicast
        if let Err(e) = self.outgoing_msg_sender.send(msg) {
            error!("failed to send outgoing message from router: {}", e);
        };
    }
}