use aex::connection::{global::GlobalContext, scope::NetworkScope};
use std::sync::Arc;
use zz_account::address::FreeWebMovementAddress;

use crate::{io_storage::IOStorage, node};

pub async fn handle(_args: Vec<String>, context: Arc<GlobalContext>) {
    let io_storage = match context.get::<IOStorage>().await {
        Some(ios) => ios,
        None => {
            eprintln!("Error: IOStorage not found in context");
            return;
        }
    };

    match io_storage.read::<FreeWebMovementAddress>("address").await {
        Some(addr) => println!("Node address: {}", addr),
        None => eprintln!("Error: Failed to read or generate node address"),
    }

    let mut total_ips = 0usize;
    let mut intranet_conns = 0usize;
    let mut extranet_conns = 0usize;
    let mut total_clients = 0usize;
    let mut total_servers = 0usize;

    for bucket_ref in context.manager.connections.iter() {
        let (key, bi_conn) = bucket_ref.pair();
        if !node::is_public_ip(&key.0) {
            continue;
        }
        total_ips += 1;
        let client_count = bi_conn.clients.len();
        let server_count = bi_conn.servers.len();
        match key.1 {
            NetworkScope::Intranet => intranet_conns += client_count + server_count,
            _ => extranet_conns += client_count + server_count,
        }
        total_clients += client_count;
        total_servers += server_count;
    }

    let total_conns = total_clients + total_servers;
    println!("\
┏━━━━━━━━━━━━━━━━ AEX Connection Profile ━━━━━━━━━━━━━━━┓
┃  Nodes (IPs):      {: <40} ┃
┃  Total Conns:      {: <40} ┃
┠──────────────────────────────────────────────────────┨
┃  Direction:        Inbound: {: <10} Outbound: {: <10} ┃
┃  Network Scope:    Intra:   {: <10} Extra:    {: <10} ┃
┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛",
        total_ips, total_conns, total_clients, total_servers, intranet_conns, extranet_conns);
}
