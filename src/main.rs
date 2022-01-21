extern crate clap;
use clap::{Arg, App, SubCommand};
use gundb::{Node, NodeConfig};
use gundb::types::GunValue;
use std::env; // TODO use clap

#[tokio::main]
async fn main() {
    env_logger::init();
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
                          .subcommand(SubCommand::with_name("serve")
                                      .about("runs the gun server")
                                      .arg(Arg::with_name("debug")
                                          .short("d")
                                          .help("print debug information verbosely")))
                          .get_matches();

    let config = matches.value_of("config").unwrap_or("default.conf");
    println!("Value for config: {}", config);

    if let Some(matches) = matches.subcommand_matches("serve") {
        if matches.is_present("debug") {
            println!("Printing debug info...");
        }

        let mut outgoing_websocket_peers = Vec::new();
        if let Ok(peers) = env::var("PEERS") {
            outgoing_websocket_peers.push(peers);
        }

        let rust_channel_size: usize = match env::var("RUST_CHANNEL_SIZE") {
            Ok(p) => p.parse::<usize>().unwrap(),
            _ => 10
        };

        let websocket_server_port: u16 = match env::var("PORT") {
            Ok(p) => p.parse::<u16>().unwrap(),
            _ => 4944
        };

        let mut node = Node::new_with_config(NodeConfig {
            outgoing_websocket_peers,
            rust_channel_size,
            websocket_server_port,
            ..NodeConfig::default()
        });

        let mut fasd = node.get("asdf").get("fasd");
        fasd.put("test".into());
        let mut sub = fasd.on();

        if let Ok(value) = sub.recv().await {
            if let GunValue::Text(str) = value {
                println!("fasd value {}", &str);
            }
        }

        node.start_adapters().await;
    }
}
