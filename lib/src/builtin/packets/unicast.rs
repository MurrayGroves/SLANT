use crate::node::{NodeData, NodeID};
use crate::packets::Packet;
use crate::propagation_models::PropagationParams;
use crate::traits::Coord;
use std::fmt::{Debug, Formatter};

/// A packet which can only be received by a specific node
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
