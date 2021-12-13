extern crate clap;
use clap::{Arg, App, SubCommand};
use std::process::Command;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    env_logger::init();
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

    if let Some(_matches) = matches.subcommand_matches("serve") {
        loop {
            let path = std::env::current_exe().unwrap();
            let path = format!("{:?}", path);
            let path = &path[1..path.len() - 5];
            println!("{}/server", path);

            let mut child = Command::new(format!("{}/server", path))
                .spawn()
                .expect("failed to execute child");

            let ecode = child.wait()
                     .expect("failed to wait on child");
            sleep(Duration::from_millis(100)).await;
        }
    }
}
