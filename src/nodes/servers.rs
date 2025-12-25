use std::net::SocketAddr;
use tokio_util::bytes;
use zz_account::address::FreeWebMovementAddress;

use crate::nodes::{
    connected_servers::{self, ConnectedServer, ConnectedServers},
    net_info,
    record::NodeRecord,
    storage,
};

use bincode::config;
use bincode::serde::decode_from_slice;
use bincode::serde::encode_to_vec;

#[derive(Clone)]
pub struct Servers {
    pub inner: Vec<NodeRecord>,
    pub external: Vec<NodeRecord>,
    pub host_external_record: Vec<NodeRecord>,
    pub host_inner_record: Vec<NodeRecord>,
    pub purified_external: Vec<NodeRecord>,
    pub purified_inner: Vec<NodeRecord>,
    pub inner_connected: Vec<NodeRecord>,
    pub external_connected: Vec<NodeRecord>,
    pub connected_servers: Option<ConnectedServers>,
}

impl Servers {
    pub fn new(storage: storage::Storeage, net_info: net_info::NetInfo) -> Self {
        let mut inner = storage.read_inner_server_list().unwrap_or_default();
        let mut external = storage.read_external_server_list().unwrap_or_default();

        // 当前节点的公网记录
        let host_external_record = NodeRecord::to_list(
            net_info.public_ips(),
            net_info.port,
            crate::protocols::defines::ProtocolCapability::TCP
                | crate::protocols::defines::ProtocolCapability::UDP,
        );

        // 当前节点的内网IP

        let host_inner_record = NodeRecord::to_list(
            net_info.local_ips(),
            net_info.port,
            crate::protocols::defines::ProtocolCapability::TCP
                | crate::protocols::defines::ProtocolCapability::UDP,
        );

        // 合并当前节点公网记录到 external（用于广播）
        external = NodeRecord::merge(host_external_record.clone(), external);

        inner = NodeRecord::merge(host_inner_record.clone(), inner);

        // 关键：生成“用于遍历的 external（不包含自己）”
        let purified_external = Self::purify_servers(&host_external_record, &external);
        let purified_inner = Self::purify_servers(&host_inner_record, &inner);

        // let connected_servers = Some(ConnectedServers::new(purified_inner.clone(), purified_external.clone()).await);

        storage
            .save_external_server_list(&external)
            .unwrap_or_default();

        storage.save_inner_server_list(&inner).unwrap_or_default();

        Self {
            inner,
            external,
            host_external_record,
            host_inner_record,
            purified_external,
            purified_inner,
            inner_connected: Vec::new(),
            external_connected: Vec::new(),
            connected_servers: None,
        }
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

    pub fn get_external_endpoints(&self) -> Vec<SocketAddr> {
        self.external.iter().map(|s| s.endpoint).collect()
    }

    /// 🔥 关键函数：
    /// 从 external 中剔除“当前节点自己的公网 IP + 端口”
    pub fn purify_servers(
        to_be_purified: &[NodeRecord],
        servers: &[NodeRecord],
    ) -> Vec<NodeRecord> {
        servers
            .iter()
            .filter(|server| {
                !to_be_purified
                    .iter()
                    .any(|host| host.endpoint == server.endpoint)
            })
            .cloned()
            .collect()
    }

    pub fn to_endpoints(records: &Vec<NodeRecord>, flag: u8) -> Vec<u8> {
        let mut endpoints: Vec<String> = records
            .iter()
            .map(|record| record.endpoint.to_string())
            .collect();

        // 内网 0 / 外网 1
        endpoints.push(flag.to_string());

        encode_to_vec(endpoints, config::standard()).expect("encode endpoints failed")
    }

    pub fn from_endpoints(endpoints: Vec<u8>) -> (Vec<SocketAddr>, u8) {
        let (mut strings, _): (Vec<String>, _) =
            decode_from_slice(&endpoints, config::standard()).expect("decode endpoints failed");

        // 弹出最后一位作为 flag
        let flag_str = strings.pop().unwrap_or_else(|| "1".to_string());
        let flag: u8 = match flag_str.as_str() {
            "0" => 0,
            _ => 1,
        };

        let endpoints: Vec<SocketAddr> = strings
            .into_iter()
            .map(|s| s.parse().expect("invalid SocketAddr"))
            .collect();

        (endpoints, flag)
    }
    /// 🔹 通知一组服务器上线
    pub async fn notify_online_servers(
        &self,
        address: FreeWebMovementAddress,
        data: &Option<Vec<u8>>,
        servers: &Vec<ConnectedServer>,
    ) {
        for server in servers {
            server
                .command
                .send_online(&address, data.clone())
                .await
                .unwrap_or_else(|e| tracing::warn!("notify_online failed: {:?}", e));
        }
    }

    /// 🔹 通知一组服务器下线
    pub async fn notify_offline_servers(
        &self,
        address: FreeWebMovementAddress,
        data: &Option<Vec<u8>>,
        servers: &Vec<ConnectedServer>,
    ) {
        for server in servers {
            server
                .command
                .send_offline(&address, data.clone())
                .await
                .unwrap_or_else(|e| tracing::warn!("notify_offline failed: {:?}", e));
        }
    }

    /// 🔹 通知所有已连接服务器当前节点的上线
    pub async fn notify_online(&self, address: FreeWebMovementAddress) -> anyhow::Result<()> {
        if let Some(connections) = &self.connected_servers {
            // inner endpoints 序列化, 0表示内网
            let mut inner_data = Servers::to_endpoints(&self.host_inner_record, 0);

            self.notify_online_servers(address.clone(), &Some(inner_data), &connections.inner)
                .await;

            // external endpoints 序列化, 1表示外网
            let mut external_data = Servers::to_endpoints(&self.host_external_record, 1);
            external_data.push(1);
            self.notify_online_servers(
                address.clone(),
                &Some(external_data),
                &connections.external,
            )
            .await;
        }
        Ok(())
    }

    /// 🔹 通知所有已连接服务器当前节点的下线
    pub async fn notify_offline(&self, address: FreeWebMovementAddress) -> anyhow::Result<()> {
        if let Some(connections) = &self.connected_servers {
            // inner endpoints 序列化
            let mut inner_data = Servers::to_endpoints(&self.host_inner_record, 0);
            self.notify_offline_servers(address.clone(), &Some(inner_data), &connections.inner)
                .await;

            // external endpoints 序列化
            let mut external_data = Servers::to_endpoints(&self.host_external_record, 1);
            self.notify_offline_servers(
                address.clone(),
                &Some(external_data),
                &connections.external,
            )
            .await;
        }
        Ok(())
    }

    pub async fn connect(&mut self) {
        self.connected_servers = Some(
            ConnectedServers::new(self.purified_inner.clone(), self.purified_external.clone())
                .await,
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::protocols::defines::ProtocolCapability;

    use super::*;
    use chrono::{DateTime, Utc};
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        vec,
    };

    // ------------------------------
    // 测试辅助：构造 NodeRecord
    // ------------------------------
    fn node(ip: [u8; 4], port: u16) -> NodeRecord {
        NodeRecord {
            endpoint: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])), port),
            protocols: ProtocolCapability::TCP,
            first_seen: Utc::now(),
            last_seen: Utc::now(),
            connected: false,
            last_disappeared: None,
            reachability_score: 100,
            address: None,
        }
    }

    // ------------------------------
    // 测试 purify_servers
    // ------------------------------
    #[test]
    fn test_purify_servers_removes_self() {
        let host = vec![node([1, 1, 1, 1], 1000), node([2, 2, 2, 2], 2000)];

        let external = vec![
            node([1, 1, 1, 1], 1000), // should be removed
            node([3, 3, 3, 3], 3000),
        ];

        let purified = Servers::purify_servers(&host, &external);

        assert_eq!(purified.len(), 1);
        assert_eq!(purified[0].endpoint, external[1].endpoint);
    }

    #[test]
    fn test_purify_servers_no_overlap() {
        let host = vec![node([1, 1, 1, 1], 1000)];

        let external = vec![node([2, 2, 2, 2], 2000), node([3, 3, 3, 3], 3000)];

        let purified = Servers::purify_servers(&host, &external);

        assert_eq!(purified.len(), 2);
    }

    #[test]
    fn test_purify_servers_empty_external() {
        let host = vec![node([1, 1, 1, 1], 1000)];
        let external = vec![];

        let purified = Servers::purify_servers(&host, &external);

        assert!(purified.is_empty());
    }

    // ------------------------------
    // 测试 add_inner / add_external
    // ------------------------------
    #[test]
    fn test_add_inner_and_external_server() {
        let mut servers = Servers {
            inner: vec![],
            external: vec![],
            host_external_record: vec![],
            host_inner_record: vec![],
            purified_external: vec![],
            purified_inner: vec![],
            inner_connected: Vec::new(),
            external_connected: Vec::new(),
            connected_servers: None,
        };

        let a = node([10, 0, 0, 1], 1111);
        let b = node([10, 0, 0, 2], 2222);

        servers.add_inner_server(a.clone());
        servers.add_external_server(b.clone());

        assert_eq!(servers.inner.len(), 1);
        assert_eq!(servers.external.len(), 1);
        assert_eq!(servers.inner[0].endpoint, a.endpoint);
        assert_eq!(servers.external[0].endpoint, b.endpoint);
    }

    // ------------------------------
    // 测试 get_all_endpoints
    // ------------------------------
    #[test]
    fn test_get_all_endpoints() {
        let servers = Servers {
            inner: vec![node([127, 0, 0, 1], 1000)],
            external: vec![node([8, 8, 8, 8], 2000)],
            host_external_record: vec![],
            host_inner_record: vec![],
            purified_external: vec![],
            purified_inner: vec![],
            inner_connected: Vec::new(),
            external_connected: Vec::new(),
            connected_servers: None,
        };

        let eps = servers.get_all_endpoints();
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn test_get_external_endpoints() {
        let servers = Servers {
            inner: vec![node([127, 0, 0, 1], 1000)],
            external: vec![node([8, 8, 8, 8], 2000), node([1, 1, 1, 1], 3000)],
            host_external_record: vec![],
            host_inner_record: vec![],
            purified_external: vec![],
            purified_inner: vec![],
            inner_connected: Vec::new(),
            external_connected: Vec::new(),
            connected_servers: None,
        };

        let eps = servers.get_external_endpoints();
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].port(), 2000);
        assert_eq!(eps[1].port(), 3000);
    }
}
