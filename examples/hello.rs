use rod::{Node, Config, Value};
use rod::adapters::*;

#[tokio::main]
async fn main() {

    let config = Config::default();
    let ws_client = OutgoingWebsocketManager::new(
        config.clone(),
        vec!["ws://localhost:4944/ws".to_string()],
    );
    let mut db = Node::new_with_config(config.clone(), vec![], vec![Box::new(ws_client)]);

    let mut sub = db.get("greeting").on();
    db.get("greeting").put("Hello World!".into());
    if let Value::Text(str) = sub.recv().await.unwrap() {
        assert_eq!(&str, "Hello World!");
        println!("{}", &str);
    }
    db.stop();
}
