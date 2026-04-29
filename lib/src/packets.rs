use crate::propagation_models::PropagationParams;
use crate::types::{Coord, NodeData, NodeID};
use enum_dispatch::enum_dispatch;
use num_traits::Num;
use std::fmt::{Debug, Formatter};
use std::hash::Hash;
use std::ops::{AddAssign, Deref};

#[derive(Clone)]
pub struct UnicastPacket {
    pub target: NodeID,
    pub content: Box<[u8]>,
}

impl Packet for UnicastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn content_ref(&self) -> &Box<[u8]> {
        &self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        Some(vec![self.target])
    }

    fn targets<A: Coord<K>, const K: usize, P: PropagationParams<A, K>>(
        &self,
        target: &NodeData<A, K, P>,
    ) -> bool {
        target.id == self.target
    }
}

impl Debug for UnicastPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.content))
    }
}

#[derive(Clone)]
pub struct MulticastPacket {
    pub content: Box<[u8]>,
}

impl Packet for MulticastPacket {
    fn content(self) -> Box<[u8]> {
        self.content
    }

    fn content_ref(&self) -> &Box<[u8]> {
        &self.content
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        None
    }

    fn targets<A: Coord<K>, const K: usize, P: PropagationParams<A, K>>(
        &self,
        target: &NodeData<A, K, P>,
    ) -> bool {
        true
    }
}

impl Debug for MulticastPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.content))
    }
}

#[enum_dispatch]
pub trait Packet: Debug + Send + Sync + Clone {
    fn content(self) -> Box<[u8]>;

    fn content_ref(&self) -> &Box<[u8]>;

    /// Returns a vec of all NodeIDs which could receive the packet if they are in range.
    /// Means the transmitter can avoid having to lookup all nodes in range.
    /// Return None if the targeting is dynamic.
    fn eager_targets(&self) -> Option<Vec<NodeID>>;

    /// Whether a packet should be received by a given node
    fn targets<A: Coord<K>, const K: usize, P: PropagationParams<A, K>>(
        &self,
        target: &NodeData<A, K, P>,
    ) -> bool
    where
        Self: Sized;
}

#[derive(Clone, Debug)]
pub enum MulticastOrUnicast {
    MulticastPacket(MulticastPacket),
    UnicastPacket(UnicastPacket),
}

impl Packet for MulticastOrUnicast {
    fn content(self) -> Box<[u8]> {
        match self {
            MulticastOrUnicast::UnicastPacket(pck) => UnicastPacket::content(pck),
            MulticastOrUnicast::MulticastPacket(pck) => <MulticastPacket as Packet>::content(pck),
        }
    }

    fn content_ref(&self) -> &Box<[u8]> {
        match self {
            MulticastOrUnicast::UnicastPacket(pck) => <UnicastPacket as Packet>::content_ref(pck),
            MulticastOrUnicast::MulticastPacket(pck) => {
                <MulticastPacket as Packet>::content_ref(pck)
            }
        }
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        match self {
            MulticastOrUnicast::UnicastPacket(pck) => <UnicastPacket as Packet>::eager_targets(pck),
            MulticastOrUnicast::MulticastPacket(pck) => {
                <MulticastPacket as Packet>::eager_targets(pck)
            }
        }
    }

    fn targets<A: Coord<K>, const K: usize, P: PropagationParams<A, K>>(
        &self,
        target: &NodeData<A, K, P>,
    ) -> bool
    where
        Self: Sized,
    {
        match self {
            MulticastOrUnicast::UnicastPacket(pck) => {
                <UnicastPacket as Packet>::targets(pck, target)
            }
            MulticastOrUnicast::MulticastPacket(pck) => {
                <MulticastPacket as Packet>::targets(pck, target)
            }
        }
    }
}

/// A packet that can provide the ID of the node which originated it
pub trait OriginatedPacket: Packet {
    /// Get ID of node which originated this packet
    fn get_origin(&self) -> NodeID;
}

/// A packet that can provide a sequence number uniquely identifying this packet w.r.t its originator
pub trait LocallySequencedPacket: OriginatedPacket {
    type S: Num + Send + Sync + Clone + Eq + Hash;
    fn seq(&self) -> Self::S;
}

/// A packet that can provide a sequence number uniquely identifying this packet globally.
/// If your packet implements [LocallySequencedPacket], you should probably just implement
/// this as a concatenation of the originator and the local sequence number.
pub trait GloballySequencedPacket: Packet {
    /// Type for sequence number
    type S: Num + Send + Sync + Copy + Eq + Hash + AddAssign + Debug;
    fn seq(&self) -> Self::S;
}
