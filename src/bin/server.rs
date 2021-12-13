use gun::Node;
use gun::types::GunValue;

#[tokio::main]
async fn main() {
    let mut node = Node::new();

    node.get("asdf").get("fasd").on(Box::new(|value: GunValue, key: String| { // TODO how to do it without Box? https://stackoverflow.com/questions/41081240/idiomatic-callbacks-in-rust
        if let GunValue::Text(str) = value {
            println!("key {} value {}", &key, &str);
        }
    }));

    node.start().await;
}