use crate::NodeConfig;

static SEEN_MSGS_MAX_SIZE: usize = 10000;

pub struct Router {
    config: Config,
    adapters: HashMap<String, Box<dyn NetworkAdapter + Send + Sync>>,
    seen_messages: BoundedHashSet,
    seen_get_messages: BoundedHashMap<String, SeenGetMessage>,
    subscribers_by_topic: HashMap<String, HashSet<String>>,
    msg_counter: AtomicUsize,
    peer_id: String,
    outgoing_msg_sender: broadcast::Sender<Message>,
    outgoing_msg_receiver: broadcast::Receiver<Message>, // need to store 1 receiver instance so the channel doesn't get closed
    incoming_msg_sender: Option<mpsc::Sender<Message>>,
    subscriptions_by_node_id: HashMap<String, Vec<broadcast::Sender<GunValue>>>
}

impl Actor for Router {
    fn new(receiver: mpsc::Receiver<Message>, addr: Weak<Addr>, node: Node) -> Self {
        let outgoing_channel = broadcast::channel::<Message>(config.rust_channel_size);
        let router = Self {
            config,
            adapters: HashMap::new(),
            seen_messages: BoundedHashSet::new(SEEN_MSGS_MAX_SIZE),
            seen_get_messages: BoundedHashMap::new(SEEN_MSGS_MAX_SIZE),
            subscribers_by_topic: HashMap::new(),
            peer_id: random_string(16),
            msg_counter: AtomicUsize::new(0),
            outgoing_msg_sender: outgoing_channel.0,
            outgoing_msg_receiver: outgoing_channel.1,
            incoming_msg_sender: None,
            subscriptions_by_node_id: Arc::new(RwLock::new(HashMap::new()))
        };
        if config.multicast {
            let multicast = Multicast::new(node.clone());
            router.adapters.write().unwrap().insert("multicast".to_string(), Box::new(multicast));
        }
        if config.websocket_server {
            let server = WebsocketServer::new(node.clone());
            router.adapters.write().unwrap().insert("ws_server".to_string(), Box::new(server));
        }
        if config.sled_storage {
            let sled_storage = SledStorage::new(node.clone());
            router.adapters.write().unwrap().insert("sled_storage".to_string(), Box::new(sled_storage));
        }
        if config.memory_storage {
            let memory_storage = MemoryStorage::new(node.clone());
            router.adapters.write().unwrap().insert("memory_storage".to_string(), Box::new(memory_storage));
        }
        let client = WebsocketClient::new(node.clone());
        router.adapters.write().unwrap().insert("ws_client".to_string(), Box::new(client));
        // should we start right away?
        node
    }

    /// Listen to incoming messages and start [NetworkAdapter]s
    async fn start(&mut self) {
        let mut node = self.clone();
        tokio::task::spawn(async move {
            loop {
                if let Some(msg) = incoming_rx.recv().await {
                    debug!("incoming message");
                    match msg {
                        Message::Put(put) => node.handle_put(put),
                        Message::Get(get) => node.handle_get(get),
                        _ => {}
                    }
                }
            }
        });

        let adapters = self.adapters.read().unwrap();
        let mut futures = Vec::new();
        for adapter in adapters.values() {
            futures.push(adapter.start()); // adapters must be non-blocking: use async functions or spawn_blocking
        }
        if self.config.read().unwrap().stats {
            self.update_stats();
        }

        let joined = futures::future::join_all(futures);

        futures::future::select(joined, Box::pin(self.stop_signal_sender.subscribe().recv())).await;
    }
}

impl Router {
    fn update_stats(&self, node: Node) {
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
        self.seen_get_messages.write().unwrap().insert(msg.id.clone(), seen_get_message);
        let topic = msg.node_id.split("/").next().unwrap_or("");
        debug!("{} subscribed to {}", msg.from, topic);
        self.subscribers_by_topic.write().unwrap().entry(topic.to_string())
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
                if let Some(seen_get_message) = self.seen_get_messages.write().unwrap().get_mut(in_response_to) {
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
                    if let Some(subscribers) = self.subscribers_by_topic.read().unwrap().get(topic) {
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

        if self.seen_messages.read().unwrap().contains(id) {
            debug!("already seen message {}", id);
            return true;
        }
        self.seen_messages.write().unwrap().insert(id.clone());

        return false;
    }

    fn send(&self, msg: Message) {
        debug!("msg out {:?}", msg);
        self.seen_messages.write().unwrap().insert(msg.get_id()); // TODO: doesn't seem to work, at least on multicast
        if let Err(e) = self.outgoing_msg_sender.send(msg) {
            error!("failed to send outgoing message from router: {}", e);
        };
    }
}