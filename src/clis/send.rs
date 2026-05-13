use std::sync::Arc;

use crate::protocols::commands::message::{next_request_id, send_text_message};
use aex::connection::global::GlobalContext;
use zz_account::address::FreeWebMovementAddress;

pub async fn handle(args: Vec<String>, context: Arc<GlobalContext>) {
    if args.len() < 2 {
        println!("Usage: send <address> <message>");
        return;
    }
    let receiver = args[0].clone();
    let msg = args[1].clone();
    let request_id = next_request_id();

    let sender = context
        .get::<FreeWebMovementAddress>()
        .await
        .map(|a| a.to_string())
        .unwrap_or_default();

    let receiver_for_closure = receiver.clone();
    context
        .manager
        .notify(receiver.as_bytes(), |entries| async move {
            for entry in entries {
                let _ = send_text_message(
                    sender.clone(),
                    receiver_for_closure.clone(),
                    request_id,
                    entry.context.clone().expect("Context missing"),
                    &msg,
                )
                .await;
            }
        })
        .await;
}
