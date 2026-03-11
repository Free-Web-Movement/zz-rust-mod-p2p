use crate::{node::Node, protocols::commands::message::send_text_message};
use std::sync::Arc;
use tokio::sync::Mutex;

pub async fn handle(node: Arc<Mutex<Node>>, args: Vec<String>) {
    if args.len() < 2 {
        println!("Usage: send <address> <message>");
        return;
    }
    let receiver = args[0].clone();
    let msg = args[1].clone();

    let n = node.lock().await;
    let receiver_clone = receiver.clone();
    let _ = n
        .context
        .manager
        .notify(receiver.as_bytes(), |entries| async move {
            for entry in entries {
                let _ = send_text_message(
                    receiver_clone.clone(),
                    entry.context.clone().expect("Context missing"),
                    &msg,
                )
                .await;
            }
        })
        .await;
}
