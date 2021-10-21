extern crate clap;
use clap::{Arg, App, SubCommand};
use gun_rs::Node;

fn main() {
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
        let _ = Node::new();
    }
}
