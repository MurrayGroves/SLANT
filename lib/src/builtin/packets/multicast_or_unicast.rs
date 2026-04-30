use crate::builtin::packets::multicast::MulticastPacket;
use crate::builtin::packets::unicast::UnicastPacket;
use crate::node::{NodeData, NodeID};
use crate::packets::Packet;
use crate::propagation_models::PropagationParams;
use crate::traits::Coord;
/// A packet which can be either multicast or unicast
#[derive(Clone, Debug)]
pub enum MulticastOrUnicast {
    MulticastPacket(MulticastPacket),
    UnicastPacket(UnicastPacket),
}

impl Packet for MulticastOrUnicast {
    fn content(self) -> Box<[u8]> {
        match self {
            MulticastOrUnicast::MulticastPacket(p) => p.content(),
            MulticastOrUnicast::UnicastPacket(p) => p.content(),
        }
    }

    fn content_ref(&self) -> &Box<[u8]> {
        match self {
            MulticastOrUnicast::MulticastPacket(p) => p.content_ref(),
            MulticastOrUnicast::UnicastPacket(p) => p.content_ref(),
        }
    }

    fn eager_targets(&self) -> Option<Vec<NodeID>> {
        match self {
            MulticastOrUnicast::MulticastPacket(p) => p.eager_targets(),
            MulticastOrUnicast::UnicastPacket(p) => p.eager_targets(),
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
            MulticastOrUnicast::UnicastPacket(p) => p.targets(target),
            MulticastOrUnicast::MulticastPacket(p) => p.targets(target),
        }
    }
}
