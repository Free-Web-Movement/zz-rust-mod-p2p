use std::sync::Arc;

use crate::protocols::commands::message::send_text_message;
use aex::connection::global::GlobalContext;

pub async fn handle(args: Vec<String>, context: Arc<GlobalContext>) {
    if args.len() < 2 {
        println!("Usage: send <address> <message>");
        return;
    }
    let receiver = args[0].clone();
    let msg = args[1].clone();

    let receiver_clone = receiver.clone();
    context
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
