use crate::Coord;
use crate::node::{NodeData, NodeID};
use crate::packets::Packet;
use crate::propagation_models::PropagationParams;
use std::fmt::{Debug, Formatter};

/// A packet which can be received by any node
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
        _target: &NodeData<A, K, P>,
    ) -> bool {
        true
    }
}

impl Debug for MulticastPacket {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&format!("{:?}", self.content))
    }
}
