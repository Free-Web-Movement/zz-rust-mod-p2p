use std::time::{SystemTime, UNIX_EPOCH};

/* =========================
   NODE
========================= */

#[derive(Clone)]
pub struct Node {
    pub id: String,
    pub ip: String,
    pub stun_port: u16,
    pub turn_port: u16,
    pub last_seen: u64,
}

impl Node {
    pub fn new(id: &str, ip: &str, stun_port: u16, turn_port: u16) -> Self {
        Self {
            id: id.to_string(),
            ip: ip.to_string(),
            stun_port,
            turn_port,
            last_seen: timestamp(),
        }
    }
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/* =========================
   TEST
========================= */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_init() {
        let n = Node::new("node1", "127.0.0.1", 3000, 3001);

        assert_eq!(n.id, "node1");
        assert_eq!(n.ip, "127.0.0.1");
        assert!(n.last_seen > 0);
    }
}
