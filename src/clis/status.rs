use aex::{connection::global::GlobalContext};
use std::sync::Arc;
use zz_account::address::FreeWebMovementAddress;

use crate::io_storage::IOStorage;

pub async fn handle(_args: Vec<String>, context: Arc<GlobalContext>) {

    let io_storage = match context.get::<IOStorage>().await {
        Some(ios) => ios,
        None => {
            eprintln!("Error: IOStorage not found in context");
            return;
        }
    };

    // 注意：read 是异步的，且返回 Option
    match io_storage
        .read::<FreeWebMovementAddress>("address")
        .await
    {
        Some(addr) => println!("Node address: {}", addr),
        None => eprintln!("Error: Failed to read or generate node address"),
    }

    let status = context.manager.status();
    println!("Status: {:?}", status);
}
