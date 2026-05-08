use aex::connection::{global::GlobalContext, scope::NetworkScope};
use std::sync::Arc;

use crate::node;

pub async fn handle(_args: Vec<String>, context: Arc<GlobalContext>) {
    let mut total_clients = 0usize;
    let mut total_servers = 0usize;
    let mut intranet_conns = 0usize;
    let mut extranet_conns = 0usize;

    for bucket_ref in context.manager.connections.iter() {
        let (key, bi_conn) = bucket_ref.pair();
        if !node::is_public_ip(&key.0) {
            continue;
        }
        match key.1 {
            NetworkScope::Intranet => intranet_conns += bi_conn.clients.len() + bi_conn.servers.len(),
            _ => extranet_conns += bi_conn.clients.len() + bi_conn.servers.len(),
        }
        total_clients += bi_conn.clients.len();
        total_servers += bi_conn.servers.len();
    }

    println!("=== Connection Status ===");
    println!("Intranet connections: {}", intranet_conns);
    println!("Extranet connections: {}", extranet_conns);
    println!("Inbound (clients): {}", total_clients);
    println!("Outbound (servers): {}", total_servers);
}
