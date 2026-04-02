use crate::packets::{GloballySequencedPacket, Packet};
use crate::propagation_models::{PropagationModel, PropagationParams};
use crate::types::{Coord, GlobalStateManager, NodeBehaviour, NodeData, SimConfig};
use num_traits::{Num, NumCast, One, Zero};
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::AddAssign;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Flood<PT: FloodPacket<A, K> + Clone + ?Sized, A: Coord<K>, const K: usize> {
    seen_packets: Arc<Mutex<HashSet<PT::S>>>,
    coord_type: PhantomData<A>,
    /// Seq of most recent packet generated
    seq: Arc<Mutex<PT::S>>,
}

pub trait FloodPacket<A: Coord<K>, const K: usize>:
    Packet<A, K> + GloballySequencedPacket<A, K> + Clone
{
    /// Type of hop count
    type H: Num + NumCast + PartialOrd + Zero;
    fn get_hop_count(&self) -> <Self as FloodPacket<A, K>>::H;

    fn set_hop_count(&mut self, count: <Self as FloodPacket<A, K>>::H);

    fn new(
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
    H: Num + NumCast + PartialOrd + Zero + Send + Sync + Clone,
    PP: PropagationParams<A, K>,
> NodeBehaviour<A, K, PP> for Flood<PT, A, K>
where
    A: Coord<K>,
    PT: FloodPacket<A, K, H = H>,
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
        let mut seen_packets = self.seen_packets.lock().unwrap();
        for packet in incoming_packets {
            // If packet hasn't been relayed before, and remaining hop count is greater than zero, retransmit
            if !seen_packets.contains(&packet.seq())
                && packet.get_hop_count() > <PT as FloodPacket<A, K>>::H::zero()
            {
                let mut packet = packet.clone();
                packet.set_hop_count(packet.get_hop_count() - <PT as FloodPacket<A, K>>::H::one());
                seen_packets.insert(packet.seq());
                global_state_manager.transmit_packet(node_data, packet);
            }
        }
        drop(seen_packets);
        self
    }
}

impl<PT: FloodPacket<A, K> + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn new() -> Self {
        Self {
            seen_packets: Arc::new(Mutex::new(HashSet::new())),
            coord_type: PhantomData,
            seq: Arc::new(Mutex::new(PT::S::zero())),
        }
    }

    pub fn gen_packet(
        &self,
        data: &NodeData<A, K, impl PropagationParams<A, K>>,
        content: Box<[u8]>,
    ) -> PT {
        let mut seq = self.seq.lock().unwrap();
        let packet = PT::new(
            data,
            <<PT as FloodPacket<A, K>>::H as NumCast>::from(0).unwrap(),
            *seq,
            content,
        );
        *seq += PT::S::one();
        packet
    }
}

impl<PT: FloodPacket<A, K> + Clone, A: Coord<K>, const K: usize> Flood<PT, A, K> {
    pub fn seq(&self) -> PT::S {
        *self.seq.lock().unwrap()
    }
}
