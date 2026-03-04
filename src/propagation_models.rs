use crate::types::{Coord, MoveBehaviour, NodeBehaviour, NodeData};

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

pub trait PropagationParams<A: Coord<K>, const K: usize>: Clone + Sized + Send + Sync {
    /// Must return the maximum possible distance at which a transmission from this node could be heard.
    /// Used internally to prune node lookups by distance.
    fn prune_distance(&self) -> A;
}

#[derive(Clone, Copy)]
pub struct SimpleDistance;

#[derive(Clone)]
pub struct SimpleDistanceParams<A: Coord<K>, const K: usize> {
    pub transmit_distance: A,
}

impl<A: Coord<K>, const K: usize> PropagationParams<A, K> for SimpleDistanceParams<A, K> {
    fn prune_distance(&self) -> A {
        self.transmit_distance
    }
}

impl<A: Coord<K>, const K: usize> PropagationModel<A, K> for SimpleDistance {
    fn signal_received(
        &self,
        sender: &NodeData<A, K, Self::P>,
        receiver: &NodeData<A, K, Self::P>,
    ) -> bool {
        let dist_sq = sender
            .position
            .iter()
            .zip(receiver.position)
            .map(|(a, b)| *a - b)
            .sum::<A>();

        dist_sq < sender.propagation_params.transmit_distance
    }

    type P = SimpleDistanceParams<A, K>;
}
