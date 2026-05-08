use aex::connection::global::GlobalContext;
use std::sync::Arc;
use zz_account::address::FreeWebMovementAddress;

use crate::{io_storage::IOStorage, node};

pub async fn handle(args: Vec<String>, context: Arc<GlobalContext>) {
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");

    let io_storage = match context.get::<IOStorage>().await {
        Some(ios) => ios,
        None => {
            eprintln!("Error: IOStorage not found in context");
            return;
        }
    };

    let address = match io_storage.read::<FreeWebMovementAddress>("address").await {
        Some(addr) => addr,
        None => {
            eprintln!("Error: Failed to read address");
            return;
        }
    };

    println!("=== Node Information ===");
    println!("Address: {}", address);
    println!("Local: {}", context.addr);

    if verbose {
        let mut total_clients = 0usize;
        let mut total_servers = 0usize;
        for bucket_ref in context.manager.connections.iter() {
            let (key, bi_conn) = bucket_ref.pair();
            if !node::is_public_ip(&key.0) {
                continue;
            }
            total_clients += bi_conn.clients.len();
            total_servers += bi_conn.servers.len();
        }
        println!("Total connections: {}", total_clients + total_servers);
        println!("Inbound (clients): {}", total_clients);
        println!("Outbound (servers): {}", total_servers);
    }
}
