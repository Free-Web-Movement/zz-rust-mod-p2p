use clap::Parser;
use tokio::io::{self, BufReader};
// src/main.rs
use zz_p2p::{cli::Opt, node::Node};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let reader = BufReader::new(stdin);
    let mut node = Node::init(Opt::parse()).await;
    node.start(reader).await;
    Ok(())
}
