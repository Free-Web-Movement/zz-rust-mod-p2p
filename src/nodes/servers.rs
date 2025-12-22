use std::net::SocketAddr;

use crate::nodes::{net_info, record::NodeRecord, storage};

pub struct Servers {
    pub inner: Vec<NodeRecord>,
    pub external: Vec<NodeRecord>,
    pub host_public_record: Vec<NodeRecord>,
}

impl Servers {
    pub fn new(storage: storage::Storeage, net_info: net_info::NetInfo) -> Self {
        let inner = storage.read_inner_server_list().unwrap_or_default();
        let mut external = storage.read_external_server_list().unwrap_or_default();
        let host_public_record = NodeRecord::to_list(
            net_info.public_ips(),
            net_info.port,
            crate::protocols::defines::ProtocolCapability::TCP | crate::protocols::defines::ProtocolCapability::UDP,
        );
        external = NodeRecord::merge(host_public_record.clone(), external);
        storage
            .save_external_server_list(&external)
            .unwrap_or_default();
        Self { inner, external, host_public_record }
    }

    pub fn add_inner_server(&mut self, server: NodeRecord) {
        self.inner.push(server);
    }

    pub fn add_external_server(&mut self, server: NodeRecord) {
        self.external.push(server);
    }

    pub fn get_all_endpoints(&self) -> Vec<SocketAddr> {
        self.inner
            .iter()
            .map(|s| s.endpoint)
            .chain(self.external.iter().map(|s| s.endpoint))
            .collect()
    }
}
