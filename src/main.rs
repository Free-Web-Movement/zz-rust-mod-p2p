// src/main.rs
use clap::Parser;
use std::sync::Arc;
use tokio::sync::Mutex;

use zz_account::address::FreeWebMovementAddress as Address;
use zz_p2p::{cli::Cli, node::Node, nodes::storage::Storeage};

#[derive(Parser, Debug)]
#[command(name = "zzp2p")]
struct Opt {
    #[arg(long, default_value = "zz-p2p-node")]
    name: String,

    #[arg(long, default_value = "0.0.0.0")]
    ip: String,

    #[arg(long, default_value_t = 9000)]
    port: u16,

    #[arg(long)]
    data_dir: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let storage = Storeage::new(opt.data_dir.as_deref(), None, None, None);

    let address = if let Some(addr) = storage.read_address()? {
        println!("Using existing address: {}", &addr);
        addr
    } else {
        let addr = Address::random();
        println!("Generated new address: {}", &addr);
        storage.save_address(&addr)?;
        addr
    };

    let node = Arc::new(Mutex::new(Node::new(
        opt.name.clone(),
        address,
        opt.ip.clone(),
        opt.port,
        Some(storage),
    )));

    {
        let node_clone = node.clone();
        tokio::spawn(async move {
            let mut n = node_clone.lock().await;
            n.start().await;
        });
    }

    println!("Node started at {}:{}", opt.ip, opt.port);

    let cli = Cli::new(node);
    cli.run().await?;

    Ok(())
}
