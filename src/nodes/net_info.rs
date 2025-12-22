use if_addrs::get_if_addrs;
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use crate::protocols::defines::ProtocolCapability;
/// 本地与公网 IP 集合
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LPIP<T> {
    pub local_ips: Vec<T>,
    pub public_ips: Vec<T>,
}

/// 节点网络信息
#[derive(Debug, Clone, PartialEq)]
pub struct NetInfo {
    pub port: u16,
    pub v4: LPIP<Ipv4Addr>,
    pub v6: LPIP<Ipv6Addr>,
    pub protocol_capabilities: ProtocolCapability,
}

impl NetInfo {
    /// 默认创建节点信息，protocol_capabilities 默认 TCP|UDP
    pub fn new(port: u16) -> Self {
        NetInfo {
            port,
            v4: LPIP {
                local_ips: Vec::new(),
                public_ips: Vec::new(),
            },
            v6: LPIP {
                local_ips: Vec::new(),
                public_ips: Vec::new(),
            },
            protocol_capabilities: ProtocolCapability::TCP | ProtocolCapability::UDP,
        }
    }

    /// 判断 IPv4 是否为公网
    pub fn is_public_v4(ip: &Ipv4Addr) -> bool {
        !(ip.is_private()
            || ip.is_loopback()
            || ip.is_link_local()
            || ip.is_multicast()
            || ip.is_broadcast()
            || ip.is_documentation())
    }

    /// 判断 IPv6 是否为公网
    pub fn is_public_v6(ip: &Ipv6Addr) -> bool {
        !(ip.is_loopback()
            || ip.is_multicast()
            || ip.is_unspecified()
            || ip.is_unique_local()
            || ip.is_unicast_link_local())
    }

    /// 添加本地 IPv4
    pub fn add_local_v4(&mut self, ip: Ipv4Addr) {
        if !self.v4.local_ips.contains(&ip) {
            self.v4.local_ips.push(ip);
        }
    }

    /// 添加本地 IPv6
    pub fn add_local_v6(&mut self, ip: Ipv6Addr) {
        if !self.v6.local_ips.contains(&ip) {
            self.v6.local_ips.push(ip);
        }
    }

    /// 添加公网 IPv4
    pub fn add_public_v4(&mut self, ip: Ipv4Addr) {
        if Self::is_public_v4(&ip) && !self.v4.public_ips.contains(&ip) {
            self.v4.public_ips.push(ip);
        }
    }

    /// 添加公网 IPv6
    pub fn add_public_v6(&mut self, ip: Ipv6Addr) {
        if Self::is_public_v6(&ip) && !self.v6.public_ips.contains(&ip) {
            self.v6.public_ips.push(ip);
        }
    }

    /// 收集节点网络信息，并判断公网 IP
    pub fn collect(port: u16) -> anyhow::Result<Self> {
        let mut v4_local = Vec::new();
        let mut v4_public = Vec::new();
        let mut v6_local = Vec::new();
        let mut v6_public = Vec::new();

        for iface in get_if_addrs()? {
            match iface.ip() {
                std::net::IpAddr::V4(ip) => {
                    if ip.is_loopback() || ip.is_link_local() {
                        continue;
                    }
                    if Self::is_public_v4(&ip) {
                        v4_public.push(ip);
                    } else {
                        v4_local.push(ip);
                    }
                }
                std::net::IpAddr::V6(ip) => {
                    if ip.is_loopback() || ip.is_unicast_link_local() {
                        continue;
                    }
                    if Self::is_public_v6(&ip) {
                        v6_public.push(ip);
                    } else {
                        v6_local.push(ip);
                    }
                }
            }
        }

        Ok(NetInfo {
            port,
            v4: LPIP {
                local_ips: v4_local,
                public_ips: v4_public,
            },
            v6: LPIP {
                local_ips: v6_local,
                public_ips: v6_public,
            },
            protocol_capabilities: ProtocolCapability::TCP | ProtocolCapability::UDP,
        })
    }

    /// 将所有公网 IP 组合成地址列表，方便加入服务器列表
    pub fn public_ips(&self) -> Vec<IpAddr> {
        let mut ips = Vec::new();
        ips.extend(self.v4.public_ips.iter().map(|ip| IpAddr::V4(*ip)));
        ips.extend(self.v6.public_ips.iter().map(|ip| IpAddr::V6(*ip)));
        ips
    }

    pub fn local_ips(&self) -> Vec<IpAddr> {
        let mut ips = Vec::new();
        ips.extend(self.v4.local_ips.iter().map(|ip| IpAddr::V4(*ip)));
        ips.extend(self.v6.local_ips.iter().map(|ip| IpAddr::V6(*ip)));
        ips
    }

    pub fn node_endpoints(&self) -> HashSet<SocketAddr> {
        let mut set = HashSet::new();

        for ip in self.public_ips() {
            set.insert(SocketAddr::new(ip, self.port));
        }
        for ip in self.local_ips() {
            set.insert(SocketAddr::new(ip, self.port));
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    /* ---------- is_public_v4 ---------- */

    #[test]
    fn test_is_public_v4() {
        assert!(!NetInfo::is_public_v4(&Ipv4Addr::new(127, 0, 0, 1))); // loopback
        assert!(!NetInfo::is_public_v4(&Ipv4Addr::new(192, 168, 1, 1))); // private
        assert!(!NetInfo::is_public_v4(&Ipv4Addr::new(224, 0, 0, 1))); // multicast
        assert!(NetInfo::is_public_v4(&Ipv4Addr::new(8, 8, 8, 8))); // public
    }

    /* ---------- is_public_v6 ---------- */

    #[test]
    fn test_is_public_v6() {
        assert!(!NetInfo::is_public_v6(&Ipv6Addr::LOCALHOST));
        assert!(!NetInfo::is_public_v6(&Ipv6Addr::UNSPECIFIED));
        assert!(!NetInfo::is_public_v6(&"fc00::1".parse().unwrap())); // unique local
        assert!(NetInfo::is_public_v6(
            &"2001:4860:4860::8888".parse().unwrap()
        )); // public
    }

    /* ---------- add_local / add_public ---------- */

    #[test]
    fn test_add_local_and_public_v4() {
        let mut info = NetInfo::new(8080);
        let local = Ipv4Addr::new(192, 168, 1, 10);
        let public = Ipv4Addr::new(8, 8, 8, 8);

        info.add_local_v4(local);
        info.add_local_v4(local); // duplicate
        assert_eq!(info.v4.local_ips.len(), 1);

        info.add_public_v4(public);
        info.add_public_v4(public); // duplicate
        assert_eq!(info.v4.public_ips.len(), 1);

        // should not accept private as public
        info.add_public_v4(local);
        assert_eq!(info.v4.public_ips.len(), 1);
    }

    #[test]
    fn test_add_local_and_public_v6() {
        let mut info = NetInfo::new(8080);
        let local: Ipv6Addr = "fe80::1".parse().unwrap();
        let public: Ipv6Addr = "2001:db8::1".parse().unwrap();

        info.add_local_v6(local);
        info.add_local_v6(local);
        assert_eq!(info.v6.local_ips.len(), 1);

        info.add_public_v6(public);
        info.add_public_v6(public);
        assert_eq!(info.v6.public_ips.len(), 1);

        info.add_public_v6(local);
        assert_eq!(info.v6.public_ips.len(), 1);
    }

    /* ---------- new() ---------- */

    #[test]
    fn test_new_default() {
        let info = NetInfo::new(9000);
        assert_eq!(info.port, 9000);
        assert!(info.v4.local_ips.is_empty());
        assert!(info.v6.public_ips.is_empty());
        assert!(info.protocol_capabilities.contains(ProtocolCapability::TCP));
        assert!(info.protocol_capabilities.contains(ProtocolCapability::UDP));
    }

    /* ---------- public_ips() ---------- */

    #[test]
    fn test_public_ips_mix() {
        let mut info = NetInfo::new(8080);

        let v4 = Ipv4Addr::new(8, 8, 8, 8);
        let v6: Ipv6Addr = "2001:4860::8888".parse().unwrap();

        info.v4.public_ips.push(v4);
        info.v6.public_ips.push(v6);

        let ips = info.public_ips();
        assert_eq!(ips.len(), 2);
        assert!(ips.contains(&IpAddr::V4(v4)));
        assert!(ips.contains(&IpAddr::V6(v6)));
    }

    #[test]
    fn test_public_ips_empty() {
        let info = NetInfo::new(8080);
        assert!(info.public_ips().is_empty());
    }

    /* ---------- collect() ---------- */

    #[test]
    fn test_collect_does_not_panic() -> anyhow::Result<()> {
        let info = NetInfo::collect(7777)?;
        assert_eq!(info.port, 7777);

        // 不对 IP 做断言（环境相关）
        let _ = info.public_ips();
        Ok(())
    }
}
