use aex::{connection::global::GlobalContext, storage::Storage};
use zz_account::address::FreeWebMovementAddress;
use std::sync::Arc;

use crate::io_storage::IOStorage;

pub async fn handle(_args: Vec<String>, context: Arc<GlobalContext>) {
    let storage = context.get::<Arc<Storage>>().await.unwrap();
    let io_storage = context.get::<IOStorage>().await.unwrap();
    println!("Node address: {}", io_storage.read::<FreeWebMovementAddress>("address", (*storage).clone()).await.unwrap());
    let status = context.manager.status();
    println!("Status: {:?}", status);
}
