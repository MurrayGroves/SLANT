use crate::types::{Coord, MoveBehaviour, NodeBehaviour, NodeData};
use log::trace;
use std::f32::consts::PI;

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

#[derive(Clone)]
pub struct FreeSpace;

impl<A: Coord<K>, const K: usize> PropagationModel<A, K> for FreeSpace {
    fn signal_received(
        &self,
        transmitter: &NodeData<A, K, <Self as PropagationModel<A, K>>::P>,
        receiver: &NodeData<A, K, <Self as PropagationModel<A, K>>::P>,
    ) -> bool {
        let tp = &transmitter.propagation_params;
        let rp = &receiver.propagation_params;

        if rp.wave_length != tp.wave_length {
            return false;
        }

        let mut direction: [A; K] = [A::default(); K];
        for x in 0..K {
            direction[x] = receiver.position[x] - transmitter.position[x];
        }

        let dist = direction
            .iter()
            .map(|x| x.powf(A::from(2.0).unwrap()))
            .sum::<A>()
            .sqrt();

        // Friis transmission equation
        let receive_power = tp.transmit_power
            + (A::from(20.0).unwrap())
                * A::log10(tp.wave_length / (A::from(4.0 * PI).unwrap() * dist))
            + (tp.transmit_gain)(transmitter, direction)
            + (rp.receive_gain)(receiver, direction);

        let received = receive_power > rp.mds;

        trace!("Transmit from {:?} to {:?} was {}", tp, rp, received);
        received
    }
    type P = FreeSpaceParams<A, K>;
}

#[derive(Clone, Debug)]
pub struct FreeSpaceParams<A: Coord<K>, const K: usize> {
    pub transmit_power: A,
    pub wave_length: A,
    /// Function which returns the transmit gain for this transmitter given the current node data and direction vector
    pub transmit_gain: fn(&NodeData<A, K, Self>, [A; K]) -> A,
    /// Function which returns the receive gain for this transmitter given the current node data and direction vector
    pub receive_gain: fn(&NodeData<A, K, Self>, [A; K]) -> A,
    /// Minimum detectable signal when this node is receiving (in dBm)
    pub mds: A,
    pub max_theoretical_range: A,
}

impl<A: Coord<K>, const K: usize> FreeSpaceParams<A, K> {
    pub fn new(
        transmit_power: A,
        wave_length: A,
        transmit_gain: fn(&NodeData<A, K, Self>, [A; K]) -> A,
        max_transmit_gain: A,
        receive_gain: fn(&NodeData<A, K, Self>, [A; K]) -> A,
        mds: A,
    ) -> Self {
        let max_theoretical_range = (wave_length
            / A::from(10.0).unwrap().powf(
                (A::from(-130).unwrap()
                    - max_transmit_gain
                    - transmit_power
                    - A::from(30.0).unwrap())
                    / A::from(20.0).unwrap(),
            ))
            / A::from(4.0 * PI).unwrap();

        trace!("Max theoretical range: {:?}", max_theoretical_range);

        Self {
            transmit_power,
            wave_length,
            transmit_gain,
            receive_gain,
            mds,
            // Calculated assuming MDS of -130dbm for a receiver with 30dBi gain.
            max_theoretical_range,
        }
    }
}

impl<A: Coord<K>, const K: usize> PropagationParams<A, K> for FreeSpaceParams<A, K> {
    fn prune_distance(&self) -> A {
        self.max_theoretical_range
    }
}
