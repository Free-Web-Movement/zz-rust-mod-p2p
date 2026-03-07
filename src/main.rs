use aex::{
    connection::global::GlobalContext, server::HTTPServer, storage::Storage, tcp::router::Router,
};
// src/main.rs
use clap::Parser;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;
use zz_p2p::{
    cli::Cli, consts::DEFAULT_APP_DIR_ADDRESS_JSON_FILE, node::Node, protocols::{command::P2PCommand, frame::P2PFrame}
};

use aex::tcp::types::Command;

use zz_account::address::FreeWebMovementAddress as Address;

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

    let storage = Storage::new(opt.data_dir.as_deref());

    let address = if let Some(addr) = storage
        .read(DEFAULT_APP_DIR_ADDRESS_JSON_FILE.to_string())
        .unwrap()
    {
        tracing::info!("Using existing address: {}", &addr);
        addr
    } else {
        let addr = Address::random();
        tracing::info!("Generated new address: {}", &addr);
        storage.save("address".to_string(), &addr).unwrap();
        addr
    };

    let addr = format!("{}:{}", opt.ip.clone(), opt.port)
        .parse::<SocketAddr>()
        .unwrap();

    let global = Arc::new(Mutex::new(GlobalContext::new(addr)));
    let node = Arc::new(Mutex::new(Node::new(
        opt.name.clone(),
        address,
        addr,
        storage.clone(),
        global.clone(),
    )));

    let server = HTTPServer::new(addr);

    let router = Router::new();

    server
        .tcp(router)
        .start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id()))
        .await?;

    // let node = Arc::new(Mutex::new(Node::new(
    //     opt.name.clone(),
    //     address,
    //     opt.ip.clone(),
    //     opt.port,
    //     Some(storage),
    // )));

    // {
    //     let node_clone = node.clone();
    //     tokio::spawn(async move {
    //         let mut n = node_clone.lock().await;
    //         n.start().await;
    //     });
    // }

    // println!("Node started at {}:{}", opt.ip, opt.port);

    let cli = Cli::new(node);
    cli.run().await?;

    Ok(())
}
