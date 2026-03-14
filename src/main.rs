use clap::Parser;
// src/main.rs
use zz_p2p::{cli::Opt, node::Node};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut node = Node::init(Opt::parse()).await;
    node.start().await;
    Ok(())
}
