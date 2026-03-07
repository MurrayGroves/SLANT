use crate::propagation_models::PropagationParams;
use crate::types::{Coord, NodeData, NodeID};
use std::fmt::{Debug, Formatter};

pub struct UnicastPacket {
    pub target: NodeID,
    pub content: Box<[u8]>,
}

impl<A: Coord<K>, const K: usize> Packet<A, K> for UnicastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn content_ref(&self) -> &Box<[u8]> {
        &self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        Some(vec![self.target])
    }

    fn targets<P: PropagationParams<A, K>>(&self, target: &NodeData<A, K, P>) -> bool {
        target.id == self.target
    }
}

impl Debug for UnicastPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.content))
    }
}

pub struct MulticastPacket {
    pub content: Box<[u8]>,
}

impl<A: Coord<K>, const K: usize> Packet<A, K> for MulticastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn content_ref(&self) -> &Box<[u8]> {
        &self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        None
    }

    fn targets<P: PropagationParams<A, K>>(&self, target: &NodeData<A, K, P>) -> bool {
        true
    }
}

impl Debug for MulticastPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.content))
    }
}

pub trait Packet<A: Coord<K>, const K: usize>: Debug + Send + Sync {
    fn content(self) -> Box<[u8]>;

    fn content_ref(&self) -> &Box<[u8]>;

    /// Returns a vec of all NodeIDs which could receive the packet if they are in range.
    /// Means the transmitter can avoid having to lookup all nodes in range.
    /// Return None if the targeting is dynamic.
    fn eager_targets(&self) -> Option<Vec<NodeID>>;

    /// Whether a packet should be received by a given node
    fn targets<P: PropagationParams<A, K>>(&self, target: &NodeData<A, K, P>) -> bool
    where
        Self: Sized;
}
