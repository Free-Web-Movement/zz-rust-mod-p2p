use std::{collections::HashSet, env::args};

use aex::storage::Storage;
use zz_account::address::FreeWebMovementAddress;

use crate::{
    consts::{
        DEFAULT_APP_DIR_ADDRESS_JSON_FILE, DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE,
        DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE,
    },
    record::NodeRecord,
};

#[derive(Debug, Clone)]
pub struct StoredFiles {
    pub storage: Storage,
    pub address_file: String,
    pub inner_server_file: String,
    pub external_server_file: String,
}

impl StoredFiles {
    pub fn new(
        storage: Storage,
        address_file: Option<String>,
        inner_server_file: Option<String>,
        external_server_file: Option<String>,
    ) -> Self {

        println!("{:?}", args());
        println!("{:?}", args());
        println!("{:?}", args());
        Self {
            storage,
            address_file: address_file.unwrap_or(DEFAULT_APP_DIR_ADDRESS_JSON_FILE.to_string()),
            inner_server_file: inner_server_file
                .unwrap_or(DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE.to_string()),
            external_server_file: external_server_file
                .unwrap_or(DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE.to_string()),
        }
    }

    pub fn address(self) -> FreeWebMovementAddress {
                println!("{:?}", &self.address_file);

        if let Some(addr) = self.storage.read(&self.address_file).unwrap() {
            tracing::info!("Using existing address: {}", &addr);
            addr
        } else {
            let addr = FreeWebMovementAddress::random();
            tracing::info!("Generated new address: {}", &addr);
            self.storage.save(&self.address_file, &addr).unwrap();
            addr
        }
    }

    pub async fn nodes(&self, is_inner: bool) -> HashSet<NodeRecord> {
        let mut path = &self.inner_server_file;
        if !is_inner {
            path = &self.external_server_file;
        }
        println!("reading nodes from {}", path);
        match self.storage.read::<HashSet<NodeRecord>>(path) {
            Ok(Some(set)) => set,
            _ => {
                let v = HashSet::new();
                self.storage.save(path, &v).unwrap();
                v
            }
        }
    }
}
