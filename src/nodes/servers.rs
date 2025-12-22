use std::net::SocketAddr;

use crate::nodes::{ net_info, record::NodeRecord, storage };
use chrono::{ DateTime, Utc };

pub struct Servers {
    pub inner: Vec<NodeRecord>,
    pub external: Vec<NodeRecord>,
    pub host_public_record: Vec<NodeRecord>,
    pub purified_external: Vec<NodeRecord>,
}

impl Servers {
    pub fn new(storage: storage::Storeage, net_info: net_info::NetInfo) -> Self {
        let inner = storage.read_inner_server_list().unwrap_or_default();
        let mut external = storage.read_external_server_list().unwrap_or_default();

        // 当前节点的公网记录
        let host_public_record = NodeRecord::to_list(
            net_info.public_ips(),
            net_info.port,
            crate::protocols::defines::ProtocolCapability::TCP |
                crate::protocols::defines::ProtocolCapability::UDP
        );

        // 合并当前节点公网记录到 external（用于广播）
        external = NodeRecord::merge(host_public_record.clone(), external);

        // 关键：生成“用于遍历的 external（不包含自己）”
        let purified_external = Self::purify_external_servers(&host_public_record, &external);

        storage.save_external_server_list(&external).unwrap_or_default();

        Self {
            inner,
            external,
            host_public_record,
            purified_external,
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
        self.external
            .iter()
            .map(|s| s.endpoint)
            .collect()
    }

    /// 🔥 关键函数：
    /// 从 external 中剔除“当前节点自己的公网 IP + 端口”
    pub fn purify_external_servers(
        host_public_record: &[NodeRecord],
        external_servers: &[NodeRecord]
    ) -> Vec<NodeRecord> {
        external_servers
            .iter()
            .filter(|server| {
                !host_public_record.iter().any(|host| host.endpoint == server.endpoint)
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{ protocols::defines::ProtocolCapability };

    use super::*;
    use std::net::{ IpAddr, Ipv4Addr, SocketAddr };
    use chrono::{ DateTime, Utc };

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
        }
    }

    // ------------------------------
    // 测试 purify_external_servers
    // ------------------------------
    #[test]
    fn test_purify_external_servers_removes_self() {
        let host = vec![node([1, 1, 1, 1], 1000), node([2, 2, 2, 2], 2000)];

        let external = vec![
            node([1, 1, 1, 1], 1000), // should be removed
            node([3, 3, 3, 3], 3000)
        ];

        let purified = Servers::purify_external_servers(&host, &external);

        assert_eq!(purified.len(), 1);
        assert_eq!(purified[0].endpoint, external[1].endpoint);
    }

    #[test]
    fn test_purify_external_servers_no_overlap() {
        let host = vec![node([1, 1, 1, 1], 1000)];

        let external = vec![node([2, 2, 2, 2], 2000), node([3, 3, 3, 3], 3000)];

        let purified = Servers::purify_external_servers(&host, &external);

        assert_eq!(purified.len(), 2);
    }

    #[test]
    fn test_purify_external_servers_empty_external() {
        let host = vec![node([1, 1, 1, 1], 1000)];
        let external = vec![];

        let purified = Servers::purify_external_servers(&host, &external);

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
            host_public_record: vec![],
            purified_external: vec![],
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
            host_public_record: vec![],
            purified_external: vec![],
        };

        let eps = servers.get_all_endpoints();
        assert_eq!(eps.len(), 2);
    }

    #[test]
    fn test_get_external_endpoints() {
        let servers = Servers {
            inner: vec![node([127, 0, 0, 1], 1000)],
            external: vec![node([8, 8, 8, 8], 2000), node([1, 1, 1, 1], 3000)],
            host_public_record: vec![],
            purified_external: vec![],
        };

        let eps = servers.get_external_endpoints();
        assert_eq!(eps.len(), 2);
        assert_eq!(eps[0].port(), 2000);
        assert_eq!(eps[1].port(), 3000);
    }
}
