extern crate clap;
use clap::{Arg, App, SubCommand};
use gundb::{Node, NodeConfig};
use std::env;
use ctrlc;

#[tokio::main]
async fn main() {
    let default_port = NodeConfig::default().websocket_server_port.to_string();
    let matches = App::new("Gun")
    .version("1.0")
    .author("Martti Malmi")
    .about("Gun node runner")
    .arg(Arg::with_name("config")
        .short("c")
        .long("config")
        .value_name("FILE")
        .help("Sets a custom config file")
        .takes_value(true))
    .subcommand(SubCommand::with_name("start")
        .about("runs the gun server")
        .arg(Arg::with_name("debug")
            .long("debug")
            .short("d")
            .help("print debug information verbosely"))
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

    //let config = matches.value_of("config").unwrap_or("default.conf");
    //println!("Value for config: {}", config);

    if let Some(matches) = matches.subcommand_matches("start") { // TODO: write fn to convert matches into NodeConfig
        let mut outgoing_websocket_peers = Vec::new();
        if let Some(peers) = matches.value_of("peers") {
            outgoing_websocket_peers = peers.split(",").map(|s| s.to_string()).collect();
        }

        if matches.is_present("debug") {
            println!("debug");
            env::set_var("RUST_LOG", "debug");
        }

        env_logger::init();

        let websocket_server_port: u16 = matches.value_of("port").unwrap().parse::<u16>().unwrap();

        let rust_channel_size: usize = match env::var("RUST_CHANNEL_SIZE") {
            Ok(p) => p.parse::<usize>().unwrap(),
            _ => 10
        };

        let websocket_server = matches.value_of("ws-server").unwrap() == "true";

        let mut node = Node::new_with_config(NodeConfig {
            outgoing_websocket_peers,
            rust_channel_size,
            websocket_server,
            websocket_server_port,
            multicast: matches.value_of("multicast").unwrap() == "true",
            cert_path: matches.value_of("cert-path").map(|s| s.to_string()),
            key_path: matches.value_of("key-path").map(|s| s.to_string()),
            stats: matches.value_of("stats").unwrap() == "true",
            ..NodeConfig::default()
        });

        if websocket_server {
            let url = format!("http://localhost:{}", websocket_server_port);
            println!("Gun server starting...");
            println!("Iris UI:      {}", url);
            println!("Stats:        {}/stats", url);
            println!("Gun endpoint: {}/gun", url);
        }

        ctrlc::set_handler(|| std::process::exit(0)).expect("Error setting Ctrl-C handler");

        node.start_adapters().await;
    }
}
