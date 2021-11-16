use futures_util::{SinkExt, StreamExt};
use log::*;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Error, Result},
};
use url::Url;
use std::collections::HashSet;
use crate::types::NetworkAdapter;
use crate::Node;
use async_trait::async_trait;
use log::{debug};

pub struct WebsocketClient {
    node: Node,
    urls: HashSet<String>
}

#[async_trait]
impl NetworkAdapter for WebsocketClient {
    fn new(node: Node) -> Self {
        WebsocketClient {
            node: node.clone(),
            urls: HashSet::new()
        }
    }

    async fn start(&self) {
        debug!("starting WebsocketClient\n");
    }

    fn stop(&self) {

    }

    fn send_str(&self, m: &String) -> () {

    }
}

const AGENT: &str = "Tungstenite";

async fn get_case_count() -> Result<u32> {
    let (mut socket, _) = connect_async(
        Url::parse("ws://localhost:9001/getCaseCount").expect("Can't connect to case count URL"),
    )
    .await?;
    let msg = socket.next().await.expect("Can't fetch case count")?;
    socket.close(None).await?;
    Ok(msg.into_text()?.parse::<u32>().expect("Can't parse case count"))
}

async fn update_reports() -> Result<()> {
    let (mut socket, _) = connect_async(
        Url::parse(&format!("ws://localhost:9001/updateReports?agent={}", AGENT))
            .expect("Can't update reports"),
    )
    .await?;
    socket.close(None).await?;
    Ok(())
}

async fn run_test(case: u32) -> Result<()> {
    info!("Running test case {}", case);
    let case_url =
        Url::parse(&format!("ws://localhost:9001/runCase?case={}&agent={}", case, AGENT))
            .expect("Bad testcase URL");

    let (mut ws_stream, _) = connect_async(case_url).await?;
    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_text() || msg.is_binary() {
            ws_stream.send(msg).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let total = get_case_count().await.expect("Error getting case count");

    for case in 1..=total {
        if let Err(e) = run_test(case).await {
            match e {
                Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
                err => error!("Testcase failed: {}", err),
            }
        }
    }

    update_reports().await.expect("Error updating reports");
}