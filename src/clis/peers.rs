use aex::connection::global::GlobalContext;
use std::sync::Arc;

pub async fn handle(_args: Vec<String>, context: Arc<GlobalContext>) {
    let status = context.manager.status();

    println!("=== Connection Status ===");
    println!("Total IPs: {}", status.total_ips);
    println!("Intranet connections: {}", status.intranet_conns);
    println!("Extranet connections: {}", status.extranet_conns);
    println!("Inbound (clients): {}", status.total_clients);
    println!("Outbound (servers): {}", status.total_servers);
    println!("Oldest uptime: {}s", status.oldest_uptime);
    println!("Average uptime: {}s", status.average_uptime);
}
