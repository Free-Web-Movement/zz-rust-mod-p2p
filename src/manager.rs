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
        let node = Node::new("manager", "127.0.0.1", 4000, 4001);
        let mgr = NodeManager::new(node.clone());

        assert_eq!(mgr.node.id, node.id);
    }
}
