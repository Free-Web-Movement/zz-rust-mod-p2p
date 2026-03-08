use aex::{server::HTTPServer, storage::Storage, tcp::router::Router};
// src/main.rs
use clap::Parser;
use std::{net::SocketAddr, sync::Arc};
use zz_p2p::{
    cli::Cli,
    node::Node,
    protocols::{command::P2PCommand, frame::P2PFrame, registry::register},
    stored_files::StoredFiles,
};

use aex::tcp::types::Command;

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

    #[arg(long)]
    address_file: Option<String>,

    #[arg(long)]
    inner_server_file: Option<String>,

    #[arg(long)]
    external_server_file: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();

    let storage = Storage::new(opt.data_dir.as_deref());

    let files = StoredFiles::new(
        storage,
        opt.address_file,
        opt.inner_server_file,
        opt.external_server_file,
    );

    // let address = files.address();

    let addr = format!("{}:{}", opt.ip.clone(), opt.port)
        .parse::<SocketAddr>()
        .unwrap();

    let node = Node::init(opt.name.clone(), files.clone(), addr);

    let server = {
        let guard = node.lock().await;
        HTTPServer::new(addr, Some(guard.context.clone()))
    };

    let mut router = Router::new();

    register(&mut router);

    server
        .tcp(router)
        .start::<P2PFrame, P2PCommand>(Arc::new(|c| c.id()))
        .await?;

    let cli = Cli::new(node);
    cli.run().await?;

    Ok(())
}
