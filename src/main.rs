extern crate clap;
use clap::{Arg, App, SubCommand};
use rod::{Node, Config};
use rod::adapters::{SledStorage, MemoryStorage, WsServer, WsServerConfig, OutgoingWebsocketManager, Multicast};
use rod::actor::Actor;
use ctrlc;

#[tokio::main]
async fn main() {
    let default_port = WsServerConfig::default().port.to_string();
    let matches = App::new("Rod")
    .version("1.0")
    .author("Martti Malmi")
    .about("Rod node runner")
    .arg(Arg::with_name("config")
        .short("c")
        .long("config")
        .value_name("FILE")
        .help("Sets a custom config file")
        .takes_value(true))
    .subcommand(SubCommand::with_name("start")
        .about("runs the rod server")
        .arg(Arg::with_name("ws-server")
            .long("ws-server")
            .env("WS_SERVER")
            .value_name("BOOL")
            .help("Run websocket server?")
            .default_value("true")
            .takes_value(true))
        .arg(Arg::with_name("port")
            .short("p")
            .long("port")
            .env("PORT")
            .value_name("NUMBER")
            .help("Websocket server port")
            .default_value(&default_port)
            .takes_value(true))
        .arg(Arg::with_name("cert-path")
            .long("cert-path")
            .env("CERT_PATH")
            .value_name("FILE")
            .help("TLS certificate path")
            .takes_value(true))
        .arg(Arg::with_name("key-path")
            .long("key-path")
            .env("KEY_PATH")
            .value_name("FILE")
            .help("TLS key path")
            .takes_value(true))
        .arg(Arg::with_name("peers")
            .long("peers")
            .env("PEERS")
            .value_name("URLS")
            .help("Comma-separated outgoing websocket peers (wss://...)")
            .takes_value(true))
        .arg(Arg::with_name("multicast")
            .long("multicast")
            .env("MULTICAST")
            .value_name("BOOL")
            .help("Enable multicast sync?")
            .default_value("false")
            .takes_value(true))
        .arg(Arg::with_name("memory-storage")
            .long("memory-storage")
            .env("MEMORY_STORAGE")
            .value_name("BOOL")
            .help("In-memory storage")
            .default_value("false")
            .takes_value(true))
        .arg(Arg::with_name("sled-storage")
            .long("sled-storage")
            .env("SLED_STORAGE")
            .value_name("BOOL")
            .help("Sled storage (disk+mem)")
            .default_value("true")
            .takes_value(true))
        .arg(Arg::with_name("sled-max-size")
            .long("sled-max-size")
            .env("SLED_MAX_SIZE")
            .value_name("BYTES")
            .help("Data in excess of this will be evicted based on priority")
            .takes_value(true))
        .arg(Arg::with_name("allow-public-space")
            .long("allow-public-space")
            .env("ALLOW_PUBLIC_SPACE")
            .value_name("BOOL")
            .help("Allow writes that are not content hash addressed or user-signed")
            .default_value("true")
            .takes_value(true))
        .arg(Arg::with_name("stats")
            .long("stats")
            .env("STATS")
            .value_name("BOOL")
            .help("Show stats at /stats?")
            .default_value("true")
            .takes_value(true))
    )
    .get_matches();

    if let Some(matches) = matches.subcommand_matches("start") { // TODO: write fn to convert matches into Config
        let mut outgoing_websocket_peers = Vec::new();
        if let Some(peers) = matches.value_of("peers") {
            outgoing_websocket_peers = peers.split(",").map(|s| s.to_string()).collect();
        }

        env_logger::init();

        let websocket_server_port: u16 = matches.value_of("port").unwrap().parse::<u16>().unwrap();

        let sled_max_size: Option<u64> = match matches.value_of("sled-max-size") {
            Some(v) => Some(v.parse::<u64>().unwrap()),
            _ => None
        };

        let mut network_adapters: Vec<Box<dyn Actor>> = Vec::new();
        let mut storage_adapters: Vec<Box<dyn Actor>> = Vec::new();

        let websocket_server = matches.value_of("ws-server").unwrap() == "true";

        let config = Config {
            allow_public_space: matches.value_of("allow-public-space").unwrap() != "false",
            stats: matches.value_of("stats").unwrap() == "true",
            ..Config::default()
        };

        // TODO init adapters here
        if matches.value_of("multicast").unwrap() == "true" {
            network_adapters.push(Box::new(Multicast::new(config.clone())));
        }
        if websocket_server {
            let cert_path = matches.value_of("cert-path").map(|s| s.to_string());
            let key_path = matches.value_of("key-path").map(|s| s.to_string());
            network_adapters.push(Box::new(WsServer::new_with_config(
                config.clone(),
                WsServerConfig {
                    port: websocket_server_port,
                    cert_path,
                    key_path
                }
            )));
        }
        if matches.value_of("sled-storage").unwrap() != "false" {
            storage_adapters.push(Box::new(SledStorage::new_with_config(
                config.clone(),
                sled::Config::default().path("sled_db"),
                sled_max_size
            )));
        }
        if matches.value_of("memory-storage").unwrap() == "true" {
            storage_adapters.push(Box::new(MemoryStorage::new()));
        }
        if outgoing_websocket_peers.len() > 0 {
            network_adapters.push(Box::new(OutgoingWebsocketManager::new(config.clone(), outgoing_websocket_peers)));
        }

        let node = Node::new_with_config(config, storage_adapters, network_adapters);

        if websocket_server {
            let url = format!("http://localhost:{}", websocket_server_port);
            println!("Node starting...");
            println!("Iris UI:      {}", url);
            println!("Stats:        {}/stats", url);
            println!("Websocket endpoint: {}/ws", url);
        }

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel();

        let mut node_clone = node.clone();
        let tx_mutex = std::sync::Mutex::new(Some(cancel_tx));
        ctrlc::set_handler(move || {
            node_clone.stop();
            if let Some(tx) = tx_mutex.lock().unwrap().take() {
                let _ = tx.send(()).unwrap();
            }
        }).expect("Error setting Ctrl-C handler");

        let _ = cancel_rx.await;
    }
}
