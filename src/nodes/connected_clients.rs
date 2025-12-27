use std::{collections::HashMap, sync::Arc};
use tokio::{net::TcpStream, sync::Mutex};
use std::net::SocketAddr;


#[derive(Clone)]
pub struct ConnectedClients {
    pub inner: HashMap<String, Vec<(Arc<Mutex<TcpStream>>, Vec<SocketAddr>)>>,
    pub external: HashMap<String, Vec<(Arc<Mutex<TcpStream>>, Vec<SocketAddr>)>>,
}

impl ConnectedClients {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
            external: HashMap::new(),
        }
    }

    pub fn add_inner(&mut self, address: &str, tcp: Arc<Mutex<TcpStream>>, sockets: Vec<SocketAddr>) {
        self.inner
            .entry(address.to_string())
            .or_insert_with(Vec::new)
            .push((tcp, sockets));
    }

    pub fn add_external(&mut self, address: &str, tcp: Arc<Mutex<TcpStream>>, sockets: Vec<SocketAddr>) {
        self.external
            .entry(address.to_string())
            .or_insert_with(Vec::new)
            .push((tcp, sockets));
    }

    pub fn remove_client(&mut self, address: &str) {
        self.inner.remove(address);
        self.external.remove(address);
    }
}
