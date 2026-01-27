use crate::types::{Coord, NodeData, NodeID};

struct UnicastPacket {
    target: NodeID,
    content: Box<[u8]>,
}

impl<A: Coord<K>, const K: usize> Packet<A, K> for UnicastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        Some(vec![self.target])
    }

    fn targets(&self, target: &NodeData<A, K>) -> bool {
        target.id == self.target
    }
}

struct MulticastPacket {
    content: Box<[u8]>,
}

impl<A: Coord<K>, const K: usize> Packet<A, K> for MulticastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        None
    }

    fn targets(&self, target: &NodeData<A, K>) -> bool {
        true
    }
}

pub trait Packet<A: Coord<K>, const K: usize> {
    fn content(self) -> Box<[u8]>;

    /// Returns a vec of all NodeIDs which could receive the packet if they are in range.
    /// Means the transmitter can avoid having to lookup all nodes in range.
    /// Return None if the targeting is dynamic.
    fn eager_targets(&self) -> Option<Vec<NodeID>>;

    /// Whether a packet should be received by a given node
    fn targets(&self, target: &NodeData<A, K>) -> bool;
}
