//! Allows packets to be received if below a certain distance.
use crate::Coord;
use crate::node::NodeData;
use crate::propagation_models::{PropagationModel, PropagationParams};

/// A simple propagation model which only accepts packets within a radius specified in [SimpleDistanceParams].
#[derive(Clone, Copy)]
pub struct SimpleDistance;

/// Parameters for [SimpleDistance]
#[derive(Clone, Debug)]
pub struct SimpleDistanceParams<A: Coord<K>, const K: usize> {
    /// The distance this node can transmit packets
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
