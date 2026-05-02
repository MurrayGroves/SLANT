//! Traits for packets and potential properties they can have.
use crate::Coord;
use crate::node::{NodeData, NodeID};
use crate::propagation_models::PropagationParams;
use num_traits::Num;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::AddAssign;

/// A packet which can be received by nodes in the network
pub trait Packet: Debug + Send + Sync + Clone {
    /// Should return the bytes content of the packet.
    fn content(self) -> Box<[u8]>;

    /// Should borrow the bytes content of the packet.
    fn content_ref(&self) -> &Box<[u8]>;

    /// Returns a vec of all NodeIDs which could receive the packet if they are in range.
    /// Means the transmitter can avoid having to look up all nodes in range.
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

/// A packet that can provide the ID of the node which originated it
pub trait OriginatedPacket: Packet {
    /// Get ID of node which originated this packet
    fn get_origin(&self) -> NodeID;
}

/// A packet that can provide a sequence number uniquely identifying this packet w.r.t its originator
pub trait LocallySequencedPacket: OriginatedPacket {
    /// Type for the sequence number.
    type S: Num + Send + Sync + Clone + Eq + Hash;

    /// Returns a sequence number for the packet which is unique for the originating node.
    fn seq(&self) -> Self::S;
}

/// A packet that can provide a sequence number uniquely identifying this packet globally.
/// If your packet implements [LocallySequencedPacket], you should probably just implement
/// this as a hash of the originator and the local sequence number.
pub trait GloballySequencedPacket: Packet {
    /// Type for sequence number
    type S: Num + Send + Sync + Copy + Eq + Hash + AddAssign + Debug;

    /// Returns a sequence number for the packet which is unique across the network.
    fn seq(&self) -> Self::S;
}
