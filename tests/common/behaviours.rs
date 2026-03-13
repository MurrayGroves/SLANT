use lazy_static::lazy_static;
use log::trace;
use manetsim::packets::{MulticastPacket, Packet};
use manetsim::propagation_models::{PropagationModel, PropagationParams};
use manetsim::types::{GlobalStateManager, MoveBehaviour, NodeBehaviour, NodeData, SimConfig};
use num_traits::ToPrimitive;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Monotonic {
    pub ticks_per_packet: usize,
    counter: usize,
    pub received_packets: usize,
}

impl Monotonic {
    pub fn new(ticks_per_packet: usize) -> Self {
        Monotonic {
            ticks_per_packet,
            counter: 0,
            received_packets: 0,
        }
    }
}

impl NodeBehaviour<f32, 2> for Monotonic {
    type P = dyn Packet<f32, 2>;

    fn tick<C: SimConfig<f32, 2, NB = Self>>(
        mut self,
        node_data: &NodeData<f32, 2, <C::PM as PropagationModel<f32, 2>>::P>,
        global_state_manager: &GlobalStateManager<f32, 2, C>,
        incoming_packets: &Vec<Box<Self::P>>,
    ) -> Self {
        trace!("Ticking {}", node_data.id);
        self.received_packets += incoming_packets.len();
        trace!(
            "{} received {:?}",
            node_data.id,
            incoming_packets.iter().map(|x| x.content_ref()[0])
        );
        self.counter += 1;

        if self.counter % self.ticks_per_packet == 0 {
            global_state_manager.transmit_packet(
                node_data,
                Box::new(MulticastPacket {
                    content: Box::new([node_data.id.to_u8().unwrap()]),
                }),
            )
        }

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

impl NodeBehaviour<f64, 2> for LoggerNode {
    type P = dyn Packet<f64, 2>;

    fn tick<C: SimConfig<f64, 2, NB = Self>>(
        self,
        node_data: &NodeData<f64, 2, <C::PM as PropagationModel<f64, 2>>::P>,
        global_state_manager: &GlobalStateManager<f64, 2, C>,
        incoming_packets: &Vec<Box<Self::P>>,
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
