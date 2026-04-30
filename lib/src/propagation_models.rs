use crate::types::{Coord, MoveBehaviour, NodeBehaviour, NodeData};
use log::trace;
use std::f32::consts::PI;

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

/// A simple propagation model which only accepts packets within a radius specified in [SimpleDistanceParams]
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

/// A propagation model which implements the Friis transmission equation to determine whether a packet is received based on [FreeSpaceParams].
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

        let received = receive_power >= rp.mds;
        trace!(
            "Transmit from {:?} to {:?} had receive power of {:?} / {:?}, at a distance of {:?}/{:?} (prune dist) was {}",
            transmitter.id,
            receiver.id,
            receive_power,
            rp.mds,
            dist,
            rp.prune_distance(),
            received
        );

        received
    }
    type P = FreeSpaceParams<A, K>;
}

/// Parameters for [FreeSpace]
#[derive(Clone, Debug)]
pub struct FreeSpaceParams<A: Coord<K>, const K: usize> {
    /// Transmit power in dB
    pub transmit_power: A,
    /// Wavelength in the same unit as [A]
    pub wave_length: A,
    /// Function which returns the transmit gain for this transmitter given the current node data and direction vector
    pub transmit_gain: fn(&NodeData<A, K, Self>, [A; K]) -> A,
    /// Function which returns the receive gain for this transmitter given the current node data and direction vector
    pub receive_gain: fn(&NodeData<A, K, Self>, [A; K]) -> A,
    /// Minimum detectable signal when this node is receiving (in dBm)
    pub mds: A,
    /// Calculated from parameters and an ideal receiver.
    max_theoretical_range: A,
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
                //
                (A::from(-120).unwrap() // MDS
                    - max_transmit_gain
                    - transmit_power
                    - A::from(30.0).unwrap()) // Receiver gain
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
