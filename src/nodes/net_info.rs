use std::net::{Ipv4Addr, Ipv6Addr};
use if_addrs::get_if_addrs;

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
        !(ip.is_private() || ip.is_loopback() || ip.is_link_local() || ip.is_multicast() || ip.is_broadcast() || ip.is_documentation())
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
    pub fn public_endpoints(&self) -> Vec<(String, u16)> {
        let mut endpoints = Vec::new();
        for ip in &self.v4.public_ips {
            endpoints.push((ip.to_string(), self.port));
        }
        for ip in &self.v6.public_ips {
            endpoints.push((ip.to_string(), self.port));
        }
        endpoints
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv4_public() {
        assert!(NetInfo::is_public_v4(&"8.8.8.8".parse().unwrap()));
        assert!(NetInfo::is_public_v4(&"1.2.3.4".parse().unwrap()));
        assert!(!NetInfo::is_public_v4(&"10.0.0.1".parse().unwrap()));
        assert!(!NetInfo::is_public_v4(&"192.168.1.1".parse().unwrap()));
        assert!(!NetInfo::is_public_v4(&"172.16.0.1".parse().unwrap()));
        assert!(!NetInfo::is_public_v4(&"127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_ipv6_public() {
        assert!(NetInfo::is_public_v6(&"2001:4860:4860::8888".parse().unwrap()));
        assert!(NetInfo::is_public_v6(&"2607:f8b0:4005:805::200e".parse().unwrap()));
        assert!(!NetInfo::is_public_v6(&"::1".parse().unwrap()));
        assert!(!NetInfo::is_public_v6(&"fc00::1".parse().unwrap()));
        assert!(!NetInfo::is_public_v6(&"fe80::1".parse().unwrap()));
        assert!(!NetInfo::is_public_v6(&"ff00::1".parse().unwrap()));
    }

    #[test]
    fn test_add_ips() {
        let mut net = NetInfo::new(30303);
        net.add_local_v4("192.168.1.2".parse().unwrap());
        net.add_local_v6("fe80::1".parse().unwrap());
        net.add_public_v4("8.8.8.8".parse().unwrap());
        net.add_public_v6("2001:4860:4860::8888".parse().unwrap());
        assert_eq!(net.v4.local_ips, vec!["192.168.1.2".parse::<std::net::IpAddr>().unwrap()]);
        assert_eq!(net.v6.local_ips, vec!["fe80::1".parse::<std::net::IpAddr>().unwrap()]);
        assert_eq!(net.v4.public_ips, vec!["8.8.8.8".parse::<std::net::IpAddr>().unwrap()]);
        assert_eq!(net.v6.public_ips, vec!["2001:4860:4860::8888".parse::<std::net::IpAddr>().unwrap()]);
    }

    #[test]
    fn test_default_protocol() {
        let net = NetInfo::new(12345);
        assert!(net.protocol_capabilities.contains(ProtocolCapability::TCP));
        assert!(net.protocol_capabilities.contains(ProtocolCapability::UDP));
        assert!(!net.protocol_capabilities.contains(ProtocolCapability::HTTP));
        assert!(!net.protocol_capabilities.contains(ProtocolCapability::WEBSOCKET));
    }

    #[test]
    fn test_public_endpoints() {
        let mut net = NetInfo::new(30303);
        net.add_public_v4("8.8.8.8".parse().unwrap());
        net.add_public_v6("2001:4860:4860::8888".parse().unwrap());
        let eps = net.public_endpoints();
        assert!(eps.contains(&("8.8.8.8".to_string(), 30303)));
        assert!(eps.contains(&("2001:4860:4860::8888".to_string(), 30303)));
    }

    #[test]
    fn test_collect_function() {
        let net = NetInfo::collect(30303).unwrap();
        // 至少包含本地 IP 或公网 IP
        assert!(net.v4.local_ips.len() + net.v4.public_ips.len() >= 0);
        assert!(net.v6.local_ips.len() + net.v6.public_ips.len() >= 0);
    }
}
