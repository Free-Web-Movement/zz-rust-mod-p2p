use aex::connection::global::GlobalContext;
use std::sync::Arc;

pub async fn handle(_args: Vec<String>, _context: Arc<GlobalContext>) {
    println!("Commands:");
    println!(" send <address> <message>   - send text message");
    println!(" connect <ip> <port>        - connect to a new node");
    println!(" status                     - show node status");
    println!(" exit                       - exit program");
}
