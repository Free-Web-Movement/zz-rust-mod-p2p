use aex::{connection::global::GlobalContext, storage::Storage};
use std::sync::Arc;
use zz_account::address::FreeWebMovementAddress;

use crate::io_storage::IOStorage;

pub async fn handle(_args: Vec<String>, context: Arc<GlobalContext>) {
    // 1. 获取 Storage
    let storage = match context.get::<Arc<Storage>>().await {
        Some(s) => s,
        None => {
            eprintln!("Error: Storage not found in context");
            return;
        }
    };

    // 2. 获取 IOStorage
    let io_storage = match context.get::<IOStorage>().await {
        Some(ios) => ios,
        None => {
            eprintln!("Error: IOStorage not found in context");
            return;
        }
    };

    // 3. 读取节点地址
    // 注意：read 是异步的，且返回 Option
    match io_storage
        .read::<FreeWebMovementAddress>("address", (*storage).clone())
        .await
    {
        Some(addr) => println!("Node address: {}", addr),
        None => eprintln!("Error: Failed to read or generate node address"),
    }

    // 4. 打印状态
    let status = context.manager.status();
    println!("Status: {:?}", status);
}
