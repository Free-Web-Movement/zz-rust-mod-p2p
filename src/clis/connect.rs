use crate::node::Node;
use std::{net::SocketAddr, sync::Arc};
use tokio::sync::Mutex;

pub async fn handle(node: Arc<Mutex<Node>>, args: Vec<String>) {
    if args.len() < 2 {
        println!("Usage: connect <ip> <port>");
        return;
    }
    let addr_str = format!("{}:{}", args[0], args[1]);
    match addr_str.parse::<SocketAddr>() {
        Ok(addr) => {
            let n = node.lock().await;
            let context = n.context.clone();
            match n
                .context
                .manager
                .connect(addr, context, move |_ctx, _t| async move {
                    println!("Connected to {}!", addr);
                })
                .await
            {
                Ok(_) => println!("Connection attempt started..."),
                Err(e) => println!("Failed to connect: {:?}", e),
            }
        }
        Err(_) => println!("Invalid address: {}", addr_str),
    }
}
