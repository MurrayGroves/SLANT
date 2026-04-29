use lazy_static::lazy_static;
use log::trace;
use manetsim::packets::{MulticastOrUnicast, MulticastPacket, Packet};
use manetsim::propagation_models::{PropagationModel, PropagationParams};
use manetsim::types::{
    Coord, GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeData, SimConfig,
};
use num_traits::ToPrimitive;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Monotonic<A: Coord<K>, const K: usize, P: Packet + Clone, PP: PropagationParams<A, K>> {
    packet_type: PhantomData<P>,
    pub ticks_per_packet: usize,
    counter: usize,
    pub received_packets: usize,
    gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet + Clone>
    Monotonic<A, K, P, PP>
{
    pub fn new(
        ticks_per_packet: usize,
        gen_packet: Arc<dyn Fn(&NodeData<A, K, PP>, Box<[u8]>) -> P + Send + Sync>,
    ) -> Self {
        Monotonic {
            packet_type: Default::default(),
            ticks_per_packet,
            counter: 0,
            received_packets: 0,
            gen_packet,
        }
    }
}

impl<A: Coord<K>, const K: usize, PP: PropagationParams<A, K>, P: Packet + Clone>
    NodeBehaviour<A, K, PP> for Monotonic<A, K, P, PP>
{
    type P = P;

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
        trace!("Ticking {}", node_data.id);
        self.received_packets += incoming_packets.len();
        trace!(
            "{} received {:?}",
            node_data.id,
            incoming_packets.iter().map(|x| x.content_ref()[0])
        );

        if self.counter % self.ticks_per_packet == 0 {
            global_state_manager.transmit_packet(
                node_data,
                (self.gen_packet)(node_data, Box::new(node_data.id.to_be_bytes())),
            )
        }

        self.counter += 1;

        self
    }
}

#[derive(Clone)]
pub struct StaticMovement {}

impl MoveBehaviour<f32, 2> for StaticMovement {
    fn tick<C: SimConfig<f32, 2, MB = Self>>(
        self,
        data: &NodeData<f32, 2, <C::PM as PropagationModel<f32, 2>>::P>,
        global_state_manager: &GlobalStateManager<f32, 2, C>,
    ) -> (Self, [f32; 2]) {
        (self, data.position)
    }
}

/// Node that just logs all incoming packets
#[derive(Clone)]
pub struct LoggerNode {}

impl<PP: PropagationParams<f64, 2>> NodeBehaviour<f64, 2, PP> for LoggerNode {
    type P = MulticastOrUnicast;

    fn tick<
        C: SimConfig<
                f64,
                2,
                PM = impl PropagationModel<f64, 2, P = PP>,
                NB = impl NodeBehaviour<f64, 2, PP, P = Self::P>,
            >,
    >(
        self,
        node_data: &NodeData<f64, 2, <C::PM as PropagationModel<f64, 2>>::P>,
        global_state_manager: &GlobalStateManager<f64, 2, C>,
        incoming_packets: &Vec<Self::P>,
    ) -> Self {
        trace!(
            "Node {} received packets {:?}",
            node_data.id, incoming_packets
        );
        let mut counter = packet_counter.lock().unwrap();
        *counter += incoming_packets.len();
        Self {}
    }
}

lazy_static! {
    pub static ref packet_counter: Mutex<usize> = Mutex::new(0);
}
