use crate::propagation_models::PropagationParams;
use crate::types::{Coord, NodeData, NodeID};
use num_traits::Num;
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

/// A packet that can provide the ID of the node which originated it
pub trait OriginatedPacket<A: Coord<K>, const K: usize>: Packet<A, K> {
    /// Get ID of node which originated this packet
    fn get_origin(&self) -> NodeID;
}

/// A packet that can provide a sequence number uniquely identifying this packet w.r.t its originator
pub trait LocallySequencedPacket<A: Coord<K>, const K: usize>: OriginatedPacket<A, K> {
    type T: Num + Send;
    fn seq(&self) -> Self::T;
}

/// A packet that can provide a sequence number uniquely identifying this packet globally.
/// If your packet implements [LocallySequencedPacket], you should probably just implement
/// this as a concatenation of the originator and the local sequence number.
pub trait GloballySequencedPacket<A: Coord<K>, const K: usize>: Packet<A, K> {
    type T: Num + Send;
    fn seq(&self) -> Self::T;
}
