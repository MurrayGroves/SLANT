use crate::packets::{GloballySequencedPacket, Packet};
use crate::propagation_models::PropagationModel;
use crate::types::{Coord, GlobalStateManager, NodeBehaviour, NodeData, SimConfig};
use num_traits::{Num, One, Zero};
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Clone)]
pub struct Flood<PT: GloballySequencedPacket<A, K> + Clone + ?Sized, A: Coord<K>, const K: usize> {
    seen_packets: HashSet<PT::S>,
    coord_type: PhantomData<A>,
}

pub trait FloodPacket<A: Coord<K>, const K: usize>:
    Packet<A, K> + GloballySequencedPacket<A, K> + Clone
{
    /// Type of hop count
    type H: Num + PartialOrd + Zero;
    fn get_hop_count(&self) -> <Self as FloodPacket<A, K>>::H;

    fn set_hop_count(&mut self, count: <Self as FloodPacket<A, K>>::H);
}

impl<A, const K: usize, PT> NodeBehaviour<A, K> for Flood<PT, A, K>
where
    A: Coord<K>,
    PT: FloodPacket<A, K>,
{
    type P = PT;

    fn tick<C: SimConfig<A, K, NB = impl NodeBehaviour<A, K, P = Self::P>>>(
        mut self,
        node_data: &NodeData<A, K, <C::PM as PropagationModel<A, K>>::P>,
        global_state_manager: &GlobalStateManager<A, K, C>,
        incoming_packets: &Vec<Box<Self::P>>,
    ) -> Self {
        for packet in incoming_packets {
            // If packet hasn't been relayed before, and remaining hop count is greater than zero, retransmit
            if !self.seen_packets.contains(&packet.seq())
                && packet.get_hop_count() > <PT as FloodPacket<A, K>>::H::zero()
            {
                let mut packet = packet.clone();
                packet.set_hop_count(packet.get_hop_count() - <PT as FloodPacket<A, K>>::H::one());
                self.seen_packets.insert(packet.seq());
                global_state_manager.transmit_packet(node_data, packet);
            }
        }
        self
    }
}

impl<PT: GloballySequencedPacket<A, K> + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn new() -> Self {
        Self {
            seen_packets: HashSet::new(),
            coord_type: PhantomData,
        }
    }
}
