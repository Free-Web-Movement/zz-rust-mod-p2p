use crate::node::Node;
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle(node: Arc<Mutex<Node>>, _: Vec<String>) {
    let n = node.lock().await;
    println!("Node address: {}", n.files.clone().address());
    let status = n.context.manager.status();
    println!("Status: {:?}", status);
}
