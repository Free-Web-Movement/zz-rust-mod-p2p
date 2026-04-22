use aex::connection::global::GlobalContext;
use std::sync::Arc;
use zz_account::address::FreeWebMovementAddress;

use crate::io_storage::IOStorage;

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
        let status = context.manager.status();
        println!("Total connections: {}", status.total_clients + status.total_servers);
        println!("Manager status: {:?}", status);
    }
}