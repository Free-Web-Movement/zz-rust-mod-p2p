use std::{
    any::Any,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use aex::storage::Storage;
use zz_account::address::FreeWebMovementAddress;

use crate::{
    cli::Opt,
    consts::{
        DEFAULT_APP_DIR_ADDRESS_JSON_FILE, DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE,
        DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE,
    },
    record::NodeRecord,
};

pub async fn read<T, F1, F2>(storage: Storage, file: &String, f1: F1, f2: F2) -> T
where
    T: for<'de> serde::Deserialize<'de> + serde::Serialize,
    F1: Fn(&T) + Send + Sync,
    F2: Fn(&String) -> T + Send + Sync,
{
    if let Some(v) = storage.read::<T>(file).unwrap() {
        f1(&v);
        v
    } else {
        let v = f2(file);
        storage.save::<T>(file, &v).unwrap();
        v
    }
}

// 定义一个 Trait 来包装你的逻辑，抹除具体的 T
pub trait StorageTask: Send + Sync {
    fn execute(&self, storage: &Storage, key: &str);
}

// 为具体的泛型实现这个 Trait
pub struct IOEntry<T> {
    pub file: String,
    pub f1: Box<dyn Fn(&T) + Send + Sync>,
    pub f2: Box<dyn Fn(&String) -> T + Send + Sync>,
}

#[derive(Clone)]
pub struct IOStorage {
    // Key 是文件名，Value 是被抹除类型的对象
    pub stores: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl Default for IOStorage {
    fn default() -> Self {
        Self {
            stores: HashMap::new(),
        }
    }
}

impl IOStorage {
    pub fn insert<T: Send + Sync + 'static>(
        &mut self,
        key: String,
        file: String,
        f1: Box<dyn Fn(&T) + Send + Sync>,
        f2: Box<dyn Fn(&String) -> T + Send + Sync>,
    ) {
        self.stores.insert(key, Arc::new(IOEntry { file, f1, f2 }));
    }

    pub fn get<T: 'static>(&self, key: &str) -> Option<&IOEntry<T>> {
        self.stores.get(key)?.downcast_ref::<IOEntry<T>>()
    }

    pub async fn read<T: 'static>(&self, key: &str, storage: Storage) -> Option<T>
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize,
    {
        match self.get::<T>(key) {
            Some(v) => Some(read(storage, &v.file, &v.f1, &v.f2).await),
            None => None,
        }
    }

    pub async fn save<T: 'static>(&self, t: &T, key: &str, storage: &Storage)
    where
        T: for<'de> serde::Deserialize<'de> + serde::Serialize,
    {
        match self.get::<T>(key) {
            Some(v) => {
                let _ = storage.save::<T>( &v.file, t);
            },
            None => {
            }
        }
    }
}

pub fn io_stroage_init(opt: &Opt, storage: Arc<Storage>) -> IOStorage {
    let arc_storage = storage.clone();
    let arc_storage1 = storage.clone();
    let arc_storage2 = storage.clone();
    let mut io_storage = IOStorage::default();
    io_storage.insert(
        "address".to_string(),
        opt
            .address_file
            .clone()
            .unwrap_or(DEFAULT_APP_DIR_ADDRESS_JSON_FILE.to_string()),
        Box::new(move |v| {
            tracing::info!("Using existing address: {}", v);
        }),
        Box::new(move |file: &String| {
            let addr = FreeWebMovementAddress::random();
            tracing::info!("Generated new address: {}", &addr);
            arc_storage.clone().save(&file, &addr).unwrap();
            addr
        }),
    );
    io_storage.insert(
        "inner_server".to_string(),
        opt
            .inner_server_file
            .clone()
            .unwrap_or(DEFAULT_APP_DIR_INNER_SERVER_LIST_JSON_FILE.to_string()),
        Box::new(|_v: &HashSet<NodeRecord>| {}),
        Box::new(move |file| {
            let v = HashSet::new();
            arc_storage1.clone().save(&file, &v).unwrap();
            v
        }),
    );
    io_storage.insert(
        "external_server".to_string(),
        opt.external_server_file
            .clone()
            .unwrap_or(DEFAULT_APP_DIR_EXTERNAL_SERVER_LIST_JSON_FILE.to_string()),
        Box::new(|_v: &HashSet<NodeRecord>| {}),
        Box::new(move |file| {
            let v = HashSet::new();
            arc_storage2.clone().save(&file, &v).unwrap();
            v
        }),
    );
    io_storage
}
