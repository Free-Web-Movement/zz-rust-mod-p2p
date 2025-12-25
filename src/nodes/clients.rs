use std::{collections::HashMap, sync::Arc};
use tokio::{net::TcpStream, sync::Mutex};

#[derive(Clone)]
pub struct Clients {
    pub inner: HashMap<String, Vec<Arc<Mutex<TcpStream>>>>,
    pub external: HashMap<String, Vec<Arc<Mutex<TcpStream>>>>,
}

impl Clients {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            external: HashMap::new(),
        }
    }

    pub fn add_inner(&mut self, address: &str, tcp: Arc<Mutex<TcpStream>>) {
        self.inner
            .entry(address.to_string())
            .or_insert_with(Vec::new)
            .push(tcp);
    }

    pub fn add_external(&mut self, address: &str, tcp: Arc<Mutex<TcpStream>>) {
        self.external
            .entry(address.to_string())
            .or_insert_with(Vec::new)
            .push(tcp);
    }
}
