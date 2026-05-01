//! Flood routing protocol, whereby packets are rebroadcast up to a hop limit.
use crate::behaviours::NodeBehaviour;
use crate::managers::GlobalStateManager;
use crate::node::NodeData;
use crate::packets::{GloballySequencedPacket, Packet};
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::{Coord, SimConfig};
use log::trace;
use num_traits::{Num, NumCast, One, Zero};
use std::collections::HashSet;
use std::fmt::Debug;
use std::marker::PhantomData;

/// Implements the flooding protocol.
/// New packets have a set hop count. Upon receiving a packet, a node will check if the hop count is above zero. If it is, it will decrement it and rebroadcast the packet.
#[derive(Clone)]
pub struct Flood<PT: FloodPacket + Clone + ?Sized, A: Coord<K>, const K: usize> {
    /// Sequence numbers of packets that this node has seen
    seen_packets: HashSet<PT::S>,
    coord_type: PhantomData<A>,
    /// Seq of most recent packet generated
    seq: PT::S,
    /// Hop count for new packets
    hop_count: usize,
}

/// Any packet which provides a hop count can be used with [Flood].
pub trait FloodPacket: Packet + GloballySequencedPacket + Clone {
    /// Type of hop count
    type H: Num + NumCast + PartialOrd + Zero;
    fn get_hop_count(&self) -> <Self as FloodPacket>::H;

    fn set_hop_count(&mut self, count: <Self as FloodPacket>::H);

    fn new<A: Coord<K>, const K: usize>(
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        hops: Self::H,
        seq: Self::S,
        content: Box<[u8]>,
    ) -> Self;
}

impl<
    A,
    const K: usize,
    PT,
    H: Num + NumCast + PartialOrd + Zero + Send + Sync + Clone + Debug,
    PP: PropagationParams<A, K>,
> NodeBehaviour<A, K, PP> for Flood<PT, A, K>
where
    A: Coord<K>,
    PT: FloodPacket<H = H>,
{
    type P = PT;

    fn tick<
        C: SimConfig<
                A,
                K,
                PM = impl PropagationModel<A, K, P = PP>,
                NB = impl NodeBehaviour<A, K, PP, P = Self::P>,
            >,
    >(
        mut self,
        node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Self::P>,
    ) -> Self {
        for packet in incoming_packets {
            // If packet hasn't been relayed before, and remaining hop count is greater than zero, retransmit
            if !self.seen_packets.contains(&packet.seq())
                && packet.get_hop_count() > <PT as FloodPacket>::H::zero()
            {
                let mut packet = packet.clone();
                packet.set_hop_count(packet.get_hop_count() - <PT as FloodPacket>::H::one());
                self.seen_packets.insert(packet.seq());
                trace!(
                    "{:?}: transmitting packet {:?} with {:?} hops left",
                    node_data.id,
                    packet.seq(),
                    packet.get_hop_count()
                );
                global_state_manager.transmit_packet(node_data, packet);
            } else {
                trace!("{:?} has already seen {:?}", node_data.id, packet.seq());
            }
        }
        self
    }
}

impl<PT: FloodPacket + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn new(hops: usize) -> Self {
        Self {
            seen_packets: HashSet::new(),
            coord_type: PhantomData,
            seq: PT::S::zero(),
            hop_count: hops,
        }
    }

    pub fn gen_packet(
        &mut self,
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        content: Box<[u8]>,
    ) -> PT {
        let packet = PT::new(
            data,
            <<PT as FloodPacket>::H as NumCast>::from(self.hop_count).unwrap(),
            self.seq,
            content,
        );
        self.seq += PT::S::one();
        packet
    }
}

impl<PT: FloodPacket + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn seq(&self) -> PT::S {
        self.seq
    }
}
