#[macro_use]
extern crate log;

extern crate clap;
use clap::{Arg, App, SubCommand};
use gun_rs::Node;
use gun_rs::types::GunValue;

#[tokio::main]
async fn main() {
    let matches = App::new("My Super Program")
                          .version("1.0")
                          .author("Martti Malmi")
                          .about("Gun runner")
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
        let mut node = Node::new();

        node.get("asdf").get("fasd").on(Box::new(|value: GunValue, key: String| { // TODO how to do it without Box? https://stackoverflow.com/questions/41081240/idiomatic-callbacks-in-rust
            if let GunValue::Text(str) = value {
                println!("key {} value {}", &key, &str);
            }
        }));

        node.start().await;
    }
}
