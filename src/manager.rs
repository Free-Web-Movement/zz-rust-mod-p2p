use crate::node::Node;

/* =========================
   MANAGER
========================= */

pub struct NodeManager {
    pub node: Node,
}

impl NodeManager {
    pub fn new(node: Node) -> Self {
        Self { node }
    }
}

/* =========================
   TEST
========================= */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager() {
        let address = zz_account::address::FreeWebMovementAddress::random();
        let n = Node::new("node1".to_owned(), address, "127.0.0.1".to_owned(), 3000, None);
        let mgr = NodeManager::new(n.clone());

        assert_eq!(mgr.node.name, n.name);
        assert_eq!(mgr.node.address.to_string(), n.address.to_string());
    }
}
