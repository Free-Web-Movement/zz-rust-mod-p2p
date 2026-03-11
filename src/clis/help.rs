use crate::node::Node;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle(_: Arc<Mutex<Node>>, _: Vec<String>) {
    println!("Commands:");
    println!(" send <address> <message>   - send text message");
    println!(" connect <ip> <port>        - connect to a new node");
    println!(" status                     - show node status");
    println!(" exit                       - exit program");
}
