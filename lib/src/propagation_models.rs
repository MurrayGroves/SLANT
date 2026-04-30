use crate::node::NodeData;
use crate::traits::Coord;

/// A propagation model is responsible for determining what nodes receive a transmitted packet.
pub trait PropagationModel<A: Coord<K>, const K: usize>: Clone + Sized + Send + Sync {
    /// Returns true if a signal sent by sender is received by the receiver
    fn signal_received(
        &self,
        sender: &NodeData<A, K, Self::P>,
        receiver: &NodeData<A, K, Self::P>,
    ) -> bool;

    /// Type used as propagation parameters for each node (e.g. transmit power, directionality).
    type P: PropagationParams<A, K>;
}

/// An instance of parameters for a propagation model - each node gets its own parameters that it can modify.
pub trait PropagationParams<A: Coord<K>, const K: usize>: Clone + Sized + Send + Sync {
    /// Must return the maximum possible distance at which a transmission from this node could be heard.
    /// Used internally to prune node lookups by distance.
    fn prune_distance(&self) -> A;
}
